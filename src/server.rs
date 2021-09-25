// Наши crates:
use crate::StructOpt;
use h2::server;
use http::{Response, StatusCode};
use rand::prelude::*;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
    time,
};

// Собственно наш ServerOpt. Мы можем добовлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt)]
pub struct ServerOpt {
    #[structopt(short, long)]
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
        tokio::select! {
            conn = listener.accept() => {
                // Разверням наш conn:
                let (stream: TcpStream, addr: SocketAddr) = conn?;

                // Получаем разрешение от нашего semaphore:
                let permit = semaphore.clone().acquire_owned().await?;

                // Посмотрим кто к нам подключился:
                dbg!(addr.to_string());

                // Протокол http/2 позволяет отправлять много html запросов. Наверное лучше обработаем их все:
                tokio::spawn(connect_handler(stream, addr, permit));
            },
            _ = shutdown_signal() => {
                todo!();
            },
        };
    //Ok(())
}

// Вынесеи код по обработке входящщих соединений от клиентов:
async fn connect_handler(
    stream: TcpStream,
    _addr: SocketAddr,
    semaphore_permit: OwnedSemaphorePermit,
) {
    // Создаем генератор псевдослучайных чисел:
    let mut rng: StdRng = SeedableRng::from_entropy();

    // Здесь происходит магия crate h2. Клиент и сервер пожимают руку:
    let mut h2 = server::handshake(stream).await.unwrap();

    // На не понял задание про время. Я могу ограничить время обработки запроса на рандомное время,
    // или имитировать работу worker. Ниже я сделал второе, не уверен что оптимально реализую первый вариант

    // Вот и наш http/2 multiplexint. Слушаем что скажет клиент:
    while let Some(request) = h2.accept().await {
        let (request, mut respond) = request.unwrap();
        println!("Получено сообщение: {:?}", request);
        // Остановим обработку запроса на рандомное время 100-500 мс:

        // Иметируем работу сервера. Я не уверен в правильной работе генератора случайных чисел, надо проверять:
        let time = Duration::from_millis(rng.gen_range(100..=500));
        println!("Sleep {} ms", time.as_millis());

        time::sleep(time).await;

        // Ответим серверу что все Ok:
        let response = Response::builder().status(StatusCode::OK).body(()).unwrap();
        respond.send_response(response, false).unwrap();
    }

    // Освободим разрешение от семафора:
    drop(semaphore_permit)
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Ошибка инициализации обработчика CTRL+C сигнала!");
}
