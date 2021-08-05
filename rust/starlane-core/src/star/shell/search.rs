use crate::error::Error;
use crate::frame::{Reply, ReplyKind, StarMessage, StarPattern, WindAction, WindUp, StarWind, Frame, WindHit, WindResults, WindDown};
use crate::lane::{LaneKey, LaneCommand, LaneWrapper};
use crate::message::resource::ProtoMessage;
use crate::message::{Fail, MessageId, ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use std::collections::{HashSet, HashMap};
use std::cmp::min;
use std::iter::FromIterator;
use crate::star::{StarKey, StarCommand, Wind, StarSkel};


pub static MAX_HOPS: usize = 32;

#[derive(Clone)]
pub struct StarSearchApi {
    pub tx: mpsc::Sender<StarSearchCall>,
}

impl StarSearchApi {
    pub fn new(tx: mpsc::Sender<StarSearchCall>) -> Self {
        Self { tx }
    }

    pub async fn search(&self, pattern: StarPattern ) -> Result<SearchHits,Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(StarSearchCall::Search { pattern, tx })?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await??)
    }


    pub fn on_frame(&mut self, frame: Frame, lane_key: LaneKey ) {
        self.tx.try_send(StarSearchCall::OnFrame {frame,lane_key}).unwrap_or_default();
    }
}

pub enum StarSearchCall {
    Search {
        pattern: StarPattern,
        tx: oneshot::Sender<SearchHits>,
    },

    OnFrame { frame: Frame, lane_key: LaneKey }
}

impl Call for StarSearchCall {}

pub struct StarSearchComponent {
    skel: StarSkel,
    transactions: HashMap<u64, StarSearchTransaction>,
}

impl StarSearchComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<StarSearchCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), transactions: HashMap::new() }),
            skel.star_search_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<StarSearchCall> for StarSearchComponent {
    async fn process(&mut self, call: StarSearchCall) {
        match call {
            StarSearchCall::Search { pattern , tx } => {
                self.search(pattern, tx).await;
            }
            StarSearchCall::OnFrame { frame, lane_key } => {
                self.on_frame(frame,lane_key).await;
            }
        }
    }
}

impl StarSearchComponent {

    async fn on_frame(&mut self, frame: Frame, lane_key: LaneKey) {
        match frame {
            Frame::StarWind(StarWind::Up(up)) => {
                self.land_windup_hop(up, lane_key).await;
            }
            Frame::StarWind(StarWind::Down(down)) => {
                self.process_search_transaction(down,lane_key)
            }
            _ => {
                return;
            }
        }
    }

    async fn search(&mut self, pattern: StarPattern, tx: oneshot::Sender<SearchHits>) {
        let wind = Wind {
            pattern,
            tx,
            max_hops: 16,
            action: WindAction::SearchHits
        };
        let tx = wind.tx;
        let wind_up = WindUp::new(self.skel.info.key.clone(), wind.pattern, wind.action);
        self.launch_windup_hop(wind_up, tx, Option::None).await;
    }

    async fn launch_windup_hop(
        &mut self,
        mut wind: WindUp,
        tx: oneshot::Sender<SearchHits>,
        exclude: Option<HashSet<StarKey>>,
    ) {
        let tid = self
            .skel
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let local_hit = match wind.pattern.is_match(&self.skel.info) {
            true => Option::Some(self.skel.info.key.clone()),
            false => Option::None,
        };

        let mut lanes = self.skel.lanes_api.lane_keys().await.expect("expected lanekeys");
        let mut lanes = HashSet::from_iter(lanes);

        match &exclude {
            None => {}
            Some(exclude) => {
                lanes.retain(|k| !exclude.contains(k));
            }
        }

        let transaction = StarSearchTransaction::new(
            wind.pattern.clone(),
            tx,
            lanes,
            local_hit,
        );
        self.transactions.insert(tid.clone(), transaction);

        wind.transactions.push(tid.clone());
        wind.hops.push(self.skel.info.key.clone());

        self.skel.lanes_api.broadcast_excluding(Frame::StarWind(StarWind::Up(wind)), exclude);
    }

    async fn land_windup_hop(&mut self, wind_up: WindUp, lane_key: LaneKey) {
        if wind_up.pattern.is_match(&self.skel.info) {
            if wind_up.pattern.is_single_match() {
                let hit = WindHit {
                    star: self.skel.info.key.clone(),
                    hops: wind_up.hops.len() + 1,
                };

                match wind_up.action.update(vec![hit], WindResults::None) {
                    Ok(result) => {
                        let wind_down = WindDown {
                            missed: None,
                            hops: wind_up.hops.clone(),
                            transactions: wind_up.transactions.clone(),
                            wind_up: wind_up,
                            result: result,
                        };

                        let wind = Frame::StarWind(StarWind::Down(wind_down));

                        self.skel.lanes_api.forward(lane_key,wind);
                    }
                    Err(error) => {
                        eprintln!(
                            "error when attempting to update wind_down results {}",
                            error
                        );
                    }
                }

                return;
            } else {
                // need to create a new transaction here which gathers 'self' as a HIT
            }
        }

        let hit = wind_up.pattern.is_match(&self.skel.info);

        let lanes = self.skel.lanes_api.lane_keys().await.expect("expected lanekeys");
        if wind_up.hops.len() + 1 > min(wind_up.max_hops, MAX_HOPS)
            || lanes.len() <= 1
            || !self.skel.info.kind.relay()
        {
            let hits = match hit {
                true => {
                    vec![WindHit {
                        star: self.skel.info.key.clone(),
                        hops: wind_up.hops.len().clone() + 1,
                    }]
                }
                false => {
                    vec![]
                }
            };

            match wind_up.action.update(hits, WindResults::None) {
                Ok(result) => {
                    let wind_down = WindDown {
                        missed: None,
                        hops: wind_up.hops.clone(),
                        transactions: wind_up.transactions.clone(),
                        wind_up: wind_up,
                        result: result,
                    };

                    let wind = Frame::StarWind(StarWind::Down(wind_down));

                    self.skel.lanes_api.forward( lane_key, wind).unwrap_or_default();
                }
                Err(error) => {
                    eprintln!(
                        "error encountered when trying to update WindResult: {}",
                        error
                    );
                }
            }

            return;
        }

        let mut exclude = HashSet::new();
        exclude.insert(lane_key);

        let (tx, rx) = oneshot::channel();

        let relay_wind_up = wind_up.clone();

        self.launch_windup_hop(relay_wind_up, tx, Option::Some(exclude));

        let skel = self.skel.clone();

        tokio::spawn(async move {
            let wind_result = rx.await;

            match wind_result {
                Ok(wind_result) => {
                    let hits = wind_result
                        .hits
                        .iter()
                        .map(|(star, hops)| WindHit {
                            star: star.clone(),
                            hops: hops.clone() + 1,
                        })
                        .collect();
                    match wind_up.action.update(hits, WindResults::None) {
                        Ok(result) => {
                            let wind_down = WindDown {
                                missed: None,
                                hops: wind_up.hops.clone(),
                                wind_up: wind_up.clone(),
                                transactions: wind_up.transactions.clone(),
                                result: result,
                            };
//                            command_tx.send(StarCommand::WindDown(wind_down)).await;

                            let lane = wind_down.hops.last().unwrap();
                            skel.lanes_api.forward(lane.clone(), Frame::StarWind(StarWind::Down(wind_down))).unwrap_or_default();
                        }
                        Err(error) => {
                            eprintln!("{}", error);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("{}", error);
                }
            }
        });
    }




    /*
    async fn find_lane_for_star(
        &mut self,
        star: StarKey,
        lane_tx: oneshot::Sender<Result<LaneKey, Error>>,
    ) {
        let lane = self.lane_with_shortest_path_to_star(&star);
        if let Option::Some(lane) = lane {
            if let Option::Some(lane) = lane.get_remote_star() {
                lane_tx.send(Ok(lane)).unwrap_or_default();
            } else {
                error!("not expecting lane to be a proto")
            }
        } else {
            let star_tx = self.skel.star_tx.clone();
            let (tx, rx) = oneshot::channel();
            self.search_for_star(star.clone(), tx).await;
            tokio::spawn(async move {
                match rx.await {
                    Ok(_) => {
                        star_tx
                            .try_send(StarCommand::GetLaneForStar { star, tx: lane_tx })
                            .unwrap_or_default();
                    }
                    Err(error) => {
                        lane_tx.send(Err(error.into())).unwrap_or_default();
                    }
                }
            });
        }
    }
    */



    fn process_search_transaction(&mut self, down: WindDown, lane_key: LaneKey) {
        let tid = down.transactions.last().cloned();

        if let Option::Some(tid) = tid {
            let transaction = self.transactions.get_mut(&tid);
            if let Option::Some(transaction) = transaction {

                match transaction.on_frame(Frame::StarWind(StarWind::Down(down)), lane_key)
                {
                    TransactionResult::Continue => {}
                    TransactionResult::Done => {
                        self.transactions.remove(&tid);
                    }
                }
            }
        }
    }



}

pub struct StarSearchTransaction {
    pub pattern: StarPattern,
    pub reported_lanes: HashSet<StarKey>,
    pub lanes: HashSet<StarKey>,
    pub hits: HashMap<StarKey, HashMap<StarKey, usize>>,
    tx: Vec<oneshot::Sender<SearchHits>>,
    local_hit: Option<StarKey>,
}

impl StarSearchTransaction {
    pub fn new(
        pattern: StarPattern,
        tx: oneshot::Sender<SearchHits>,
        lanes: HashSet<StarKey>,
        local_hit: Option<StarKey>,
    ) -> Self {
        StarSearchTransaction {
            pattern: pattern,
            reported_lanes: HashSet::new(),
            hits: HashMap::new(),
            tx: vec![tx],
            lanes: lanes,
            local_hit: local_hit,
        }
    }

    fn collapse(&self) -> HashMap<StarKey, usize> {
        let mut rtn = HashMap::new();
        for (_lane, map) in &self.hits {
            for (star, hops) in map {
                if rtn.contains_key(star) {
                    if let Some(old) = rtn.get(star) {
                        if hops < old {
                            rtn.insert(star.clone(), hops.clone());
                        }
                    }
                } else {
                    rtn.insert(star.clone(), hops.clone());
                }
            }
        }

        if let Option::Some(local) = &self.local_hit {
            rtn.insert(local.clone(), 0);
        }

        rtn
    }

    pub async fn commit(&mut self) {
        if self.tx.len() != 0 {
            let tx = self.tx.remove(0);
            let commit = WindCommit {
                tx: tx,
                result: SearchHits {
                    pattern: self.pattern.clone(),
                    hits: self.collapse(),
                    lane_hits: self.hits.clone(),
                },
            };

            unimplemented!()
//            self.command_tx.send(StarCommand::WindCommit(commit)).await;
        }
    }
}

impl StarSearchTransaction {
    fn on_lane_closed(&mut self, key: &StarKey) -> TransactionResult {
        self.lanes.remove(key);
        self.reported_lanes.remove(key);

        if self.reported_lanes == self.lanes {
            self.commit();
            TransactionResult::Done
        } else {
            TransactionResult::Continue
        }
    }

    fn on_frame(
        &mut self,
        frame: Frame,
        lane_key: LaneKey
    ) -> TransactionResult {

        if let Frame::StarWind(StarWind::Down(wind_down)) = frame {
            if let WindResults::Hits(hits) = &wind_down.result {
                let mut lane_hits = HashMap::new();
                for hit in hits.clone() {
                    if !lane_hits.contains_key(&hit.star) {
                        lane_hits.insert(hit.star.clone(), hit.hops);
                    } else {
                        if let Option::Some(old) = lane_hits.get(&hit.star) {
                            if hit.hops < *old {
                                lane_hits.insert(hit.star.clone(), hit.hops);
                            }
                        }
                    }
                }

                self.hits.insert(lane_key.clone(), lane_hits);
            }
        }

        self.reported_lanes.insert( lane_key );

        if self.reported_lanes == self.lanes {
            self.commit();
            TransactionResult::Done
        } else {
            TransactionResult::Continue
        }
    }
}


pub struct LaneHit {
    lane: StarKey,
    star: StarKey,
    hops: usize,
}

pub struct WindCommit {
    pub result: SearchHits,
    pub tx: oneshot::Sender<SearchHits>,
}

#[derive(Clone)]
pub struct SearchHits {
    pub pattern: StarPattern,
    pub hits: HashMap<StarKey, usize>,
    pub lane_hits: HashMap<StarKey, HashMap<LaneKey, usize>>,
}

impl SearchHits {
    pub fn nearest(&self) -> Option<WindHit> {
        let mut min: Option<WindHit> = Option::None;

        for (star, hops) in &self.hits {
            if min.as_ref().is_none() || hops < &min.as_ref().unwrap().hops {
                min = Option::Some(WindHit {
                    star: star.clone(),
                    hops: hops.clone(),
                });
            }
        }

        min
    }
}

pub enum TransactionResult {
    Continue,
    Done,
}


pub struct ShortestPathStarKey {
    pub to: StarKey,
    pub next_lane: StarKey,
    pub hops: usize,
}
