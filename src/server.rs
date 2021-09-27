// Наши crates:
use crate::StructOpt;
use bytes::Bytes;
use h2::{server, server::Connection};
use http::{Response, StatusCode};
use rand::prelude::*;
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, OwnedSemaphorePermit, Semaphore},
    time::{self},
};
// Собственно наш ServerOpt. Мы можем добавлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt)]
pub struct ServerOpt {
    #[structopt(short, long)]
    bind_addr: SocketAddr,
    #[structopt(short, long, default_value = "5")]
    connect_limmit: usize,
}

// Весь код, связанный с запуском сервера, пишем тут:
pub async fn run(opt: ServerOpt) {
    // Для начала откроем socket, адрес и порт возьмем из ServerOpt:
    let listener = TcpListener::bind(opt.bind_addr).await.unwrap();

    // Для ограничения максимального подключения будем использовать Semaphore. При каждом
    // подключении клиента сервер "спрашивает разрешение"
    let semaphore = Arc::new(Semaphore::new(opt.connect_limmit));

    // Нас как-то надо уведомлять уже запущены connect_handler's о завершении работы сервера,
    // для этого я решил использовать tokoo Notify. При получении сигнала обработчик отправит всем
    // подписанным обработчикам соединений с клиентам разрешение на завершение:
    let (sender_stop, _) = broadcast::channel::<()>(1);

    // Объявим вектор, в нем будем хранить join handle соединений клиентов
    let mut join_connect_statistics = Vec::<tokio::task::JoinHandle<Duration>>::with_capacity(100);

    // Объявим Option для хранения количества прерванных соединений:
    let mut err_number_connect = None;

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
                let join = tokio::spawn(connect_handler(stream, addr, permit, stop));
                join_connect_statistics.push(join);
            },
            // Получили сигнал, пора завершать работу сервера:
            _ = shutdown_signal() => {
                // Отправляю всем подписчикам на  broadcast channel разрешение на завершение своей работы:
                if let Err(_) = sender_stop.send(()) {
                    break;
                }

                // Ожидаю 10 секунд завершения работы всех обработчиков соединений:
                tokio::select! {
                    _ = time::sleep(Duration::from_secs(10)) => {
                        println!("Время ожидания завершения сервера истекло!");
                        err_number_connect = Some(opt.connect_limmit - semaphore.available_permits());
                        break;
                    }
                    // Пытаемся захватить все разрешения доступные нам, так мы будем уверенны что нет больше работуючих connect_handler:
                    _ = semaphore.clone().acquire_many_owned(opt.connect_limmit as u32) => {
                        break;
                    }
                }
            },
        }
    }

    // Подготовим статистику и выведем ее в терминал.
    // Вообще у меня повторяется код, можно было бы вынести в отдельный mod обработку и хранение статистики, но я уже устал.

    // Посчитаем сколько запросов от клиента получили, это довольно легко:
    let connect_number = join_connect_statistics.len();

    if connect_number > 0 {
        // У нас в join_request_statistics хранятся только futures, надо из обработать:
        let mut connect_statistic = Vec::with_capacity(connect_number);

        for join in join_connect_statistics {
            connect_statistic.push(join.await.unwrap())
        }

        // Копипастить не хорошо, но Junior - мне можно:
        // Посчитаем минимальное время ответа:
        let min = connect_statistic.iter().min().unwrap_or(&Duration::ZERO);

        // Максимальное время ответа:
        let max = connect_statistic.iter().max().unwrap_or(&Duration::ZERO);

        // Среднее арифметическое время ответа, можно было бы посчитать еще медиану:
        let sum: Duration = connect_statistic.iter().sum();

        let average = sum / connect_number as u32;

        let err_number_connect = if let Some(num) = err_number_connect {
            format!("{}", num)
        } else {
            "None".to_string()
        };

        println!(
            "    
Обработано подключений:         {}
    Прервано подключений:       {}
    Минимальное время ответа:   {} мс
    Максимальное время ответа:  {} мс
    Среднее время ответа:       {} мс\n",
            connect_number,
            err_number_connect,
            min.as_millis(),
            max.as_millis(),
            average.as_millis()
        );
    }

    println!("Сервер завершил работу!");
    return;
}

// Вынес код по обработке входящих соединений от клиентов:
async fn connect_handler(
    stream: TcpStream,
    addr: SocketAddr,
    semaphore_permit: OwnedSemaphorePermit,
    mut stop_signal: broadcast::Receiver<()>,
) -> Duration {
    // Здесь происходит магия crate h2. Клиент и сервер пожимают руку:
    let mut h2: Connection<TcpStream, Bytes> =
        server::Builder::new().handshake(stream).await.unwrap();

    // Создаем генератор псевдослучайных чисел:
    let mut rng: StdRng = SeedableRng::from_entropy();

    println!("Новое подключение:              {}\n", addr.to_string());

    // Для сбора статистики у меня есть два варианта:
    //  1 При tokio::spawn записывать все JoinHandle в Vec или FuturesOrdered/FuturesUnrdered. При отключении клиента или завершении работы сервера пройтись по всем
    //    JoinHandle, получить из кдждого нуждую статистику переданную из connect_handler
    //  2 Объявить обернутый в Arc<Mutex<_>> объект, по завершении каждой connect_handler Future писать в него свою статистику.

    // Я не буду объявлять отдельную структуру под статистику, у меня и так вышел спагетти код, который хочется переписать. Я создам несколько:
    // Узнаем сколько времени мы обслуживали клиента:
    let service_time = Instant::now();

    // Объявим вектор для хранения результата обработки запросов. Он хранит в себе время обработки и удачно ли прошла обработка:
    let mut join_request_statistics = Vec::with_capacity(100);

    // Я не понял задание про время. Я могу ограничить время обработки запроса на рандомное время,
    // или имитировать работу worker. Ниже я сделал второе, не уверен что оптимально реализую первый вариант

    // Здесь мы будем следить за запросами от клиента, Если получили сигнал о завершении работы, то уведомым об этом клиентов.
    while let Some(request) = h2.accept().await {
        // Клиент может неожиданно отключиться от сервера, необходимо это обработать
        // Я не стану отлавливать причину:
        if let Ok((_request, mut respond)) = request {
            let time = Duration::from_millis(rng.gen_range(100..=500));

            let service_time = Instant::now();

            // Правильно завершить работу сервера поможет:
            if let Ok(()) = stop_signal.try_recv() {
                println!("\nПолучен сигнал завершения работы! {}", &addr.to_string());
                h2.graceful_shutdown();
            }

            // Каждый запрос обработаем отдельно. Такой подход значительно производительней:
            let join_handle = tokio::spawn(async move {
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
                    std::result::Result::<(), h2::Error>::Ok(())
                }
                .await;

                // Возвращаем сатистику:
                (service_time.elapsed(), err.is_ok())
            });

            join_request_statistics.push(join_handle);
        } else {
            eprintln!("Ошибка соединения с клиентом: {}", &addr.to_string());
            break;
        }
    }

    let service_time = service_time.elapsed();

    // В этом блоке я буду высчитывать и выводить статистику
    {
        // Посчитаем сколько запросов от клиента получили, это довольно легко:
        let requests_number = join_request_statistics.len();

        // У нас в join_request_statistics храняться только futures, надо из обработать:
        let mut request_statistic = Vec::with_capacity(requests_number);

        for join in join_request_statistics {
            request_statistic.push(join.await.unwrap())
        }

        // Посчитаем минимальное время ответа:
        let min = request_statistic
            .iter()
            //.filter(|(_time, err)| *err)
            .map(|(time, _ok)| time)
            .min()
            .unwrap_or(&Duration::ZERO);

        // Максимальное время ответа:
        let max = request_statistic
            .iter()
            //.filter(|(_time, err)| *err)
            .map(|(time, _ok)| time)
            .max()
            .unwrap_or(&Duration::ZERO);

        // Среднее арифметическое время ответа, можно было бы посчитать еще медиану:
        let sum: Duration = request_statistic
            .iter()
            //.filter(|(_time, err)| *err)
            .map(|(time, _ok)| time)
            .sum();

        let average = sum / requests_number as u32;

        // Выведем это все на терминал. Не очень изящьно, но работает:
        println!(
            "
Клиент отключился:              {}
    Общее время сeанса:         {} мс
    Поступило запросов:         {} 
    Минимальное время ответа:   {} мс
    Максимальное время ответа:  {} мс
    Среднее время ответа:       {} мс
    Общее время ответoв:        {} мс\n",
            &addr.to_string(),
            service_time.as_millis(),
            requests_number,
            min.as_millis(),
            max.as_millis(),
            average.as_millis(),
            sum.as_millis()
        );
    }

    // Явно освобождаю разрешение от семафоар:
    drop(semaphore_permit);

    // Вернем общее время обслуживания клиента
    service_time
}

// Небольшая обертка:
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Ошибка инициализации обработчика CTRL+C сигнала!");
}
