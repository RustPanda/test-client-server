// Наши crates:
use crate::StructOpt;
use bytes::Bytes;
use h2::{server, server::Connection};
use http::{Response, StatusCode};
use rand::prelude::*;
use std::{net::SocketAddr, process::exit, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, OwnedSemaphorePermit, Semaphore},
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
pub async fn run(opt: ServerOpt) {
    // Для начаоа откроем socket, адрес и порт возмем из ServerOpt:
    let listener = TcpListener::bind(opt.bind_addr).await.unwrap();

    // Для ограничения максимального подключения будем исполльзовать Semaphore. При каждом
    // подключении клиента сервер "спрашивает разрешение"
    let semaphore = Arc::new(Semaphore::new(opt.connect_limmit));

    // Нас как-то надо уведомлять уже запущщеные connect_handler's о завершении работы сервера,
    // для этого я решил использовать tokoo Notify. При получении сигнала обработчик отправит всем
    // подписаным обработчикам соединений с клиентаи разрешение на завершение:
    let (sender_stop, _) = broadcast::channel::<()>(1);

    loop {
        tokio::select! {
            // Обрабатываем новое подключение клиента:
            conn = listener.accept() => {
                // Разверням наш conn:
                let (stream, addr) = conn.unwrap();

                // Получаем разрешение от нашего semaphore:
                let permit = semaphore.clone().acquire_owned().await.unwrap();

                // Клонирую notify. У каждого оброботчика соединений будет своя копия:
                let stop = sender_stop.subscribe();

                // К нам может придти несколько клиентов по отдельности, будем обробатовать каждый асинхронно:
                tokio::spawn(connect_handler(stream, addr, permit, stop));
            },
            // Получили сигнал, пора завершать работу сервера:
            _ = shutdown_signal() => {
                // Отправляю всеи подписчикам на  broadcast channel разрешение на завершение своей работы:
                if let Err(_) =  sender_stop.send(()) {
                    println!("\nОстановка сервера...");
                    break;
                } else {
                    println!("\nАктивнх соединений: {}\nOстановка сервера...", opt.connect_limmit - semaphore.available_permits());
                }

                // Ожидаю 5 секунд завершения работы всез обработчиков соединений:
                tokio::select! {
                    _ = time::sleep(Duration::from_secs(5)) => {
                        println!("Время ожидания грациозного завершения сервера истекло!\n\n\n\n\n");
                        println!("Число клиентов, недождавшихся ответа: {}", opt.connect_limmit - semaphore.available_permits());
                        break;
                    }
                    // Пытаемся захватить все разрешения доступные нам, так мы будем уверенны что нет больше работуючих connect_handler:
                    res = semaphore.clone().acquire_many_owned(opt.connect_limmit as u32) => {
                        match res {
                            Ok(_) => {
                                println!("Все соединения с клиентами завершены!");
                                break
                            },
                            Err(err) => {
                                eprintln!("Не удалось завершить соединения с клиентами. \nError: {}", err);
                                break
                            },
                        }
                    }
                }
            },
        }
    }

    println!("Сервет завершил работу!");
    exit(0);
}

// Вынес код по обработке входящщих соединений от клиентов:
async fn connect_handler(
    stream: TcpStream,
    addr: SocketAddr,
    semaphore_permit: OwnedSemaphorePermit,
    mut stop_signal: broadcast::Receiver<()>,
) {
    // Здесь происходит магия crate h2. Клиент и сервер пожимают руку:
    let mut h2: Connection<TcpStream, Bytes> =
        server::Builder::new().handshake(stream).await.unwrap();

    // Создаем генератор псевдослучайных чисел:
    let mut rng: StdRng = SeedableRng::from_entropy();

    println!("Новое подключение: {}", addr.to_string());

    let semaphore_permit = Arc::new(semaphore_permit);

    // Я не понял задание про время. Я могу ограничить время обработки запроса на рандомное время,
    // или имитировать работу worker. Ниже я сделал второе, не уверен что оптимально реализую первый вариант

    // Здесь мы будем следить за запросами от клиента, Если получили сигнал о завершении работы, то уведомым об этом клиентов.
    while let Some(request) = h2.accept().await {
        // Клиент может неожиданно отключиться от сервера, необходимо это обработать
        // Я не стану отлавливать причину:
        if let Ok((request, mut respond)) = request {
            let time = Duration::from_millis(rng.gen_range(100..=500));

            // Правильно завершить работу сервера поможет:
            if let Ok(_) = stop_signal.try_recv() {
                println!("Получен сигнал завершения работы! {}", &addr.to_string());
                h2.graceful_shutdown();
            }

            let clone_semaphore_permit = semaphore_permit.clone();

            // Каждый запрос обработаем отдельно. Такой подход значительно производительней:
            tokio::spawn(async move {
                // Обработка запросто происходит не мгоновенно, за это время клиент может потерять соединение.
                // Отломим все ошибки и обработаем их:
                let err = async {
                    // Здесь обрабатываем поступившие запросы от клиента:
                    // Остановим обработку запроса на рандомное время 100-500 мс:
                    // println!(
                    //     "Получeн запрос: {} {} от {}. Сон: {}",
                    //     request.method(),
                    //     request.uri(),
                    //     addr.to_string(),
                    //     time.as_millis()
                    // );
                    time::sleep(time).await;

                    // Ответим серверу что все Ok:
                    let response = Response::builder().status(StatusCode::OK).body(()).unwrap();

                    let _ = respond.send_response(response, true)?;
                    std::result::Result::<(), Box<dyn std::error::Error>>::Ok(())
                }
                .await;

                // Выведем ошибку в терминал:
                if let Err(err) = err {
                    // eprintln!(
                    //     "Ошибка обработки запроса клиента: {} {}",
                    //     err,
                    //     &addr.to_string()
                    // );
                }
                drop(clone_semaphore_permit);
            });
        } else {
            eprintln!("Ошибка соединения с клиентом: {}", &addr.to_string());
            break;
        }
    }

    println!("Клиент отключился: {}", &addr.to_string());
    // Освободим разрешение от семафора:
    drop(semaphore_permit);
}

// Небольшая обертка:
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Ошибка инициализации обработчика CTRL+C сигнала!");
}
