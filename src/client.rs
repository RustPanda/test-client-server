use std::net::SocketAddr;

use h2::client;
use http::Request;
use tokio::net::TcpStream;

// Экспортируем StructOpt в server модуль, без него не сможем определить ServerOpt:
use crate::StructOpt;

// Собственно наш ClientOpt. Мы можем добовлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt, Clone, Copy)]
pub struct ClientOpt {
    #[structopt(short, long)]
    connect: SocketAddr,
}

// Весь код, связанный с запуском клиента, пишем тут:
pub async fn run(opt: ClientOpt) {
    dbg!(opt);

    let tcp = TcpStream::connect(opt.connect).await.unwrap();
    let (sender, h2) = client::handshake(tcp).await.unwrap();

    tokio::spawn(async move {
        if let Err(e) = h2.await {
            println!("GOT ERR={:?}", e);
        }
    });
    let mut jobs = Vec::new();

    for i in 0..100000 {
        let mut clone_sendr = sender.clone();

        let a = tokio::spawn(async move {
            let request = Request::builder()
                .uri(format!("https://http2.akamai.com/{}", i))
                .body(())
                .unwrap();

            println!("№ {}", i);
            let (response, _) = clone_sendr.send_request(request, true).unwrap();
            let _response = response.await.unwrap();
        });

        jobs.push(a);
    }

    for a in jobs {
        a.await.unwrap();
    }
}
