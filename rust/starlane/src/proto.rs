use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};

use futures::future::{err, join_all, ok, select_all};
use futures::FutureExt;
use futures::prelude::*;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex, broadcast, oneshot};

use crate::constellation::Constellation;
use crate::error::Error;
use crate::id::{Id, IdSeq};
use crate::lane::{STARLANE_PROTOCOL_VERSION, TunnelSenderState, Lane, TunnelConnector, TunnelSender, LaneCommand, TunnelReceiver, ConnectorController, LaneMeta};
use crate::frame::{ProtoFrame, Frame, StarMessageInner, StarMessagePayload, StarSearchInner, SearchPattern, StarSearchResultInner, StarSearchHit};
use crate::star::{Star, StarKernel, StarKey, StarKind, StarCommand, StarController, Transaction, StarSearchTransaction};
use std::cell::RefCell;
use std::collections::HashMap;
use std::task::Poll;
use crate::frame::Frame::{StarMessage, StarSearch};

pub struct ProtoStar
{
  kind: StarKind,
  key: StarKey,
  command_rx: Receiver<StarCommand>,
  lanes: HashMap<StarKey, LaneMeta>,
  connector_ctrls: Vec<ConnectorController>,
  sequence: Option<IdSeq>,
  transactions: HashMap<i64,Box<dyn Transaction>>,
  transaction_seq: AtomicI64,
  star_search_transactions: HashMap<i64,StarSearchTransaction>
}

impl ProtoStar
{
    pub fn new(key: StarKey, kind: StarKind) ->(Self, StarController)
    {
        let (command_tx, command_rx) = mpsc::channel(32);
        (ProtoStar{
            kind,
            key,
            command_rx: command_rx,
            lanes: HashMap::new(),
            connector_ctrls: vec![],
            sequence: Option::None,
            transactions: HashMap::new(),
            transaction_seq: AtomicI64::new(0),
            star_search_transactions: HashMap::new()
        }, StarController{
            command_tx: command_tx
        })
    }

    pub async fn evolve(mut self)->Result<Star,Error>
    {
        // request a sequence from central
        loop {
            let mut futures = vec!();
            futures.push(self.command_rx.recv().boxed() );

            for (key,mut lane) in &mut self.lanes
            {
               futures.push( lane.lane.incoming.recv().boxed() )
            }

            let (command,_,_) = select_all(futures).await;

            if let Some(command) = command
            {
                match command{
                    StarCommand::AddLane(lane) => {
                        self.lanes.insert(lane.remote_star.clone(), LaneMeta::new(lane));
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::Frame(frame) => {
                        println!("received frame: {}", frame);
                    }
                }
            }
            else
            {
                return Err("command_rx has been disconnected".into());
            }

        }

        Ok(Star::new( self.lanes, Box::new(PlaceholderKernel::new()) ))
    }

    async fn lane_added(&mut self)
    {
        if self.sequence.is_none()
        {
            let message = Frame::StarMessage(StarMessageInner{
                from: self.key.clone(),
                to: StarKey::central(),
                payload: StarMessagePayload::RequestSequence
            });
            self.send(&StarKey::central(), message).await
        }
    }

    async fn send(&mut self, star: &StarKey, frame: Frame )
    {
        for (remote_star,lane) in &self.lanes
        {
            if lane.has_path_to_star(star)
            {
                lane.lane.outgoing.tx.send( LaneCommand::LaneFrame(frame) ).await;
                break;
            }
        }

    }

    async fn search(&mut self, star: StarKey )
    {
        let search_id = self.transaction_seq.fetch_add(1, Ordering::Relaxed );
        let search_transaction = StarSearchTransaction::new(star.clone());
        self.star_search_transactions.insert(search_id, search_transaction );

        let search = Frame::StarSearch(StarSearchInner{
            from: self.key.clone(),
            pattern: SearchPattern::StarKey(star),
            hops: vec![self.key.clone()],
            transactions: vec![search_id],
            max_hops: 255,
            multi: false
        });

        for (star,lane) in &self.lanes
        {
           lane.lane.outgoing.tx.send( LaneCommand::LaneFrame(search.clone())).await;
        }
    }

    async fn process_frame( &mut self, frame: Frame, lane: &LaneMeta )
    {
        match frame
        {
            StarSearch(search) => {
                self.on_star_search(search, lane);
            }
            Frame::StarSearchResult(result) => {
                self.on_star_search_result(result);
            }
            StarMessage(_) => {

                eprintln!("star does not handle messages yet");
            }
            _ => {
                eprintln!("star does not handle frame: {}", frame)
            }
        }
    }

    async fn on_star_search( &mut self, search: StarSearchInner, lane: &LaneMeta )
    {
        let hit = match &search.pattern
        {
            SearchPattern::StarKey(star) => {
                self.key == *star
            }
            SearchPattern::StarKind(kind) => {
                self.kind == *kind
            }
        };

        if hit
        {
            let hops = search.hops.len()+1;
            let frame = Frame::StarSearchResult(StarSearchResultInner{
                missed: None,
                hits: vec![StarSearchHit{ star: self.key.clone(), hops: hops as _ }],
                search: search.clone(),
                transactions: search.transactions
            });

            lane.lane.outgoing.tx.send(LaneCommand::LaneFrame(frame)).await;

            if !search.multi
            {
                return;
            }
        }

    }

    async fn on_star_search_result( &mut self, search: StarSearchResultInner)
    {

    }

}

pub struct ProtoStarController
{
    command_tx: Sender<StarCommand>
}


#[derive(Clone)]
pub enum ProtoStarKernel
{
   Central,
   Mesh,
   Supervisor,
   Server,
   Gateway
}


impl ProtoStarKernel
{
    fn evolve(&self) -> Result<Box<dyn StarKernel>, Error>
    {
        Ok(Box::new(PlaceholderKernel::new()))
    }
}


pub struct PlaceholderKernel
{

}

impl PlaceholderKernel{
    pub fn new()->Self
    {
        PlaceholderKernel{}
    }
}

impl StarKernel for PlaceholderKernel
{

}


pub struct ProtoTunnel
{
    pub star: Option<StarKey>,
    pub tx: Sender<Frame>,
    pub rx: Receiver<Frame>
}

impl ProtoTunnel
{

    pub async fn evolve(mut self) -> Result<(TunnelSender, TunnelReceiver),Error>
    {
        self.tx.send(Frame::Proto(ProtoFrame::StarLaneProtocolVersion(STARLANE_PROTOCOL_VERSION))).await;

        if let Option::Some(star)=self.star
        {
            self.tx.send(Frame::Proto(ProtoFrame::ReportStarKey(star))).await;
        }

        // first we confirm that the version is as expected
        if let Option::Some(Frame::Proto(recv)) = self.rx.recv().await
        {
            match recv
            {
                ProtoFrame::StarLaneProtocolVersion(version) if version == STARLANE_PROTOCOL_VERSION => {
                    // do nothing... we move onto the next step
                },
                ProtoFrame::StarLaneProtocolVersion(version) => {
                    return Err(format!("wrong version: {}", version).into());
                },
                gram => {
                    return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)", gram).into());
                }
            }
        }
        else {
            return Err("disconnected".into());
        }

        if let Option::Some(Frame::Proto(recv)) = self.rx.recv().await
        {

            match recv
            {
                ProtoFrame::ReportStarKey(remote_star_key) => {
                    return Ok((TunnelSender{
                        remote_star: remote_star_key.clone(),
                        tx: self.tx,
                    }, TunnelReceiver{
                        remote_star: remote_star_key.clone(),
                        rx: self.rx,
                        }));
                }
                frame => { return Err(format!("unexpected star gram: {} (expected to receive ReportStarKey next)", frame).into()); }
            };
        }
        else {
            return Err("disconnected!".into())
        }
    }


}

pub fn local_tunnels(high: StarKey, low:StarKey) ->(ProtoTunnel, ProtoTunnel)
{
    let (atx,arx) = mpsc::channel::<Frame>(32);
    let (btx,brx) = mpsc::channel::<Frame>(32);

    (ProtoTunnel {
        star: Option::Some(high),
        tx: atx,
        rx: brx
    },
     ProtoTunnel
    {
        star: Option::Some(low),
        tx: btx,
        rx: arx
    })
}
