use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("REGISTRY MAINN!");

    tokio::spawn(async move {
       println!("... FROM TOKIO");
    });

    tokio::time::sleep(Duration::from_secs(5)).await;
    println!("and done!");
}



#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
