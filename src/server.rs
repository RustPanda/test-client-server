use std::{net::SocketAddr, sync::Arc};

// Наши crates:
use crate::StructOpt;
use h2::server;
use http::{Response, StatusCode};
use tokio::{net::TcpListener, sync::Semaphore};

// Собственно наш ServerOpt. Мы можем добовлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt)]
pub struct ServerOpt {
    bind_addr: SocketAddr,
    #[structopt(short, long, default_value = "5")]
    connect_limmit: usize,
}

// Весь код, связанный с запуском сервера, пишем тут:
pub async fn run(opt: ServerOpt) -> crate::Result<()> {
    // Для начаоа откроем socket, адрес и порт возмем из ServerOpt:
    let listener = TcpListener::bind(opt.bind_addr).await?;

    // Для ограничения максимального подключения будем исполльзовать Semaphore. При каждом
    // подключении клиента сервер "спрашивает разрешение"
    let semaphore = Arc::new(Semaphore::new(opt.connect_limmit));

    loop {
        // Ждем подключение клиента:
        let (stream, addr) = listener.accept().await?;

        // Получаем разрешение от нашего semaphore:
        let permit = semaphore.clone().acquire_owned().await?;

        // Посмотрим кто к нам подключился:
        dbg!(addr.to_string());

        // Протокол http/2 позволяет потправлять много html запросов. Давайте обработаем их все:
        tokio::spawn(async {
            // Необходимо захватить наш semaphor permit, а я даже не заню как это переводится:
            let _permit = permit;

            // Здесь происходит магия crate h2. Клиент и сервер пожимают руку:
            let mut h2 = server::handshake(stream).await.unwrap();

            // Вот и наш http/2 multiplexint. Слушаем что скажет клиент:
            while let Some(request) = h2.accept().await {
                let (request, mut respond) = request.unwrap();

                println!("Получено сообщение: {:?}", request);
                let response = Response::builder().status(StatusCode::OK).body(()).unwrap();
                respond.send_response(response, false).unwrap();
            }
        });
    }

    Ok(())
}
