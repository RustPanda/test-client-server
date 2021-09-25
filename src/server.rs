// Наши crates:
use crate::StructOpt;
use h2::server;
use http::{Response, StatusCode};
use rand::prelude::*;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Notify, OwnedSemaphorePermit, Semaphore},
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

    // Нас как-то надо уведомлять уже запущщеные connect_handler's о завершении работы сервера,
    // для этого я решил использовать tokoo Notify. При получении сигнала обработчик отправит всем
    // подписаным обработчикам соединений с клиентаи разрешение на завершение:
    let stop_notify = Arc::new(Notify::new());

    loop {
        tokio::select! {
            // Обрабатываем новое подключение клиента:
            conn = listener.accept() => {
                // Разверням наш conn:
                let (stream, addr) = conn?;

                // Получаем разрешение от нашего semaphore:
                let permit = semaphore.clone().acquire_owned().await?;

                // Клонирую notify. У каждого оброботчика соединений будет своя копия:
                let clone_stop_notify = stop_notify.clone();

                // Посмотрим кто к нам подключился:
                dbg!(addr.to_string());

                // К нам может придти несколько клиентов по отдельности, будем обробатовать каждый асинхронно:
                tokio::spawn(connect_handler(stream, addr, permit, clone_stop_notify));
            },
            // Получили сигнал, пора завершать работу сервера:
            _ = shutdown_signal() => {
                // Отправляю всеи подписчикам на  nitify разрешение на завершение своей работы:
                stop_notify.notify_waiters();

                // Ожидаю 30 секунд завершения работы всез обработчиков соединений:
                tokio::select! {
                    _ = time::sleep(Duration::from_secs(30)) => {
                        println!("Время ожидания грациозного завершения сервера истекло!");
                        dbg!(semaphore.clone());
                        break
                    }
                    res = semaphore.clone().acquire_owned() => {
                        match res {
                            Ok(_) => {
                                println!("Все соединения с клиентами завершены!");
                            },
                            Err(err) => {
                                eprint!("Не удалось завершить соединения с клиентами. \nError: {}", err);
                                break
                            },
                        }
                    }
                }
            },
        }
    }

    println!("Сервет завершил работу!");
    Ok(())
}

// Вынес код по обработке входящщих соединений от клиентов:
async fn connect_handler(
    stream: TcpStream,
    addr: SocketAddr,
    semaphore_permit: OwnedSemaphorePermit,
    clone_stop_notify: Arc<Notify>,
) {
    // Зарегистрируем обработчик в Notify. Вызвав await смодет получить разрешение на завершение работы:
    let notifaid = clone_stop_notify.notified();

    // Создаем генератор псевдослучайных чисел:
    let mut rng: StdRng = SeedableRng::from_entropy();

    // Здесь происходит магия crate h2. Клиент и сервер пожимают руку:
    let mut h2 = server::handshake(stream).await.unwrap();

    // На не понял задание про время. Я могу ограничить время обработки запроса на рандомное время,
    // или имитировать работу worker. Ниже я сделал второе, не уверен что оптимально реализую первый вариант

    // Здесь мы будем следить за requests от клиента и уточнять не пора ли нам завершить свою работу.
    // Я не стану обрабатывать каждый запрос честно паралельно, в данном тэстовом задании этого не требуется.
    // Нагрузка на сервер иметируется sleep, но от у меня async. Во время сна мы блокируеи поток:
    tokio::select! {
        _ = notifaid => {
            // Мы получили сигнал о завершении работы, нам надо бообщить об этом клиентам. Для этого в h2 есть:
            h2.graceful_shutdown();
        }
        // Вот и наше не совсем паралельное http/2 multeplexing, но для этого проэка будет достаточно:
        request = h2.accept() => {
            if let Some(request)= request {
                // Здесь обрабатываем поступившие запросы от клиента:
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

            } else {
                println!("Запросов от {} больше нет!", addr.to_string());
            }

        }
    }

    // Освободим разрешение от семафора:
    drop(semaphore_permit)
}

// Небольшая обертка:
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Ошибка инициализации обработчика CTRL+C сигнала!");
}
