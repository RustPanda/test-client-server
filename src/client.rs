use h2::client;
use http::Request;
use std::{net::SocketAddr, time::Duration};
use tokio::{net::TcpStream, time};

// Экспортируем StructOpt в server модуль, без него не сможем определить ServerOpt:
use crate::StructOpt;

// Собственно наш ClientOpt. Мы можем добовлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt, Clone, Copy)]
pub struct ClientOpt {
    #[structopt(short, long)]
    connect: SocketAddr,
    #[structopt(short, long)]
    n: u32,
}

// Весь код, связанный с запуском клиента, пишем тут:
pub async fn run(opt: ClientOpt) {
    dbg!(opt);

    let stream = TcpStream::connect(opt.connect);
    let sleep = time::sleep(Duration::from_secs(2));

    tokio::select! {
        _ = sleep => {
            eprintln!("Ошибка подключения к серверу: Время ожидания истекло!")
        }

        Ok(stream) = stream => {
            println!("Соединение с сервером установлено!");
            // Вынесу обработу соединения в отдельную функцию:
            handler(stream, opt).await;
        }
    }
}

async fn handler(tcp: TcpStream, opt: ClientOpt) {
    let (sender, h2) = client::handshake(tcp).await.unwrap();

    // Магия crate h2:
    tokio::spawn(async move {
        if let Err(e) = h2.await {
            println!("GOT ERR={:?}", e);
        }
    });

    // Я упакую отправленные запросы в вектор:
    let mut tasks = Vec::new();

    // Создаеи новые h2 соединения с сервером и отправляем запросы:
    for i in 0..opt.n {
        let mut clone_sendr = sender.clone();

        //  Заголовок запроса:
        let request = Request::builder()
            .uri(format!("https://http2.akamai.com/{}", i))
            .body(())
            .unwrap();

        // Отправляем запрос:
        if let Ok((response, _)) = clone_sendr.send_request(request, true) {
            // Каждый запрос и обработка ответа асинхронны:
            let join = tokio::spawn(async move {
                let res = async {
                    // Дожидаемся ответа:
                    let response = response.await?;

                    println!("№ {} {}", i, response.status());
                    std::result::Result::<(), Box<dyn std::error::Error>>::Ok(())
                }
                .await;

                if let Err(err) = res {
                    eprintln!("Ошибка: {}", err);
                };
            });

            tasks.push(join);
        } else {
            break;
        };
    }

    // Ждем получения ответов на все запросы:
    for join in tasks {
        join.await.unwrap();
    }
}
