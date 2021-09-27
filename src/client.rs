use h2::client;
use http::Request;
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::TcpStream,
    time::{self, Instant},
};

// Экспортируем StructOpt в server модуль, без него не сможем определить ServerOpt:
use crate::StructOpt;

// Собственно наш ClientOpt. Мы можем добавлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt, Clone, Copy)]
pub struct ClientOpt {
    #[structopt(short, long, help = "Адрес сервера, к которому отправляем запросы.")]
    connect: SocketAddr,
    #[structopt(
        short,
        long,
        help = "Количество параллельных запросов. Диапазон от 1 до 100!"
    )]
    number: u32,
    #[structopt(short, long, help = "Отключить лимит для number")]
    anlimited: bool,
}

// Весь код, связанный с запуском клиента, пишем тут:
pub async fn run(opt: ClientOpt) {
    // Проверим правильный ли диапазон нам передали. Есть возможность реализовать это нативно в structopt,
    // но делать я это конечно же не буду:
    if (opt.number < 1 || opt.number > 100) && !opt.anlimited {
        eprintln!("Неверный диапазон значений для n!");
        eprintln!("Для получения подробностей выполните команду:  test-client-server client -h");
        return;
    }

    // Создаем пару futures. Первая для подключения е серверу, а вторая timeout:
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
    // Время работы клиента:
    let client_working_time = Instant::now();

    let (sender, h2) = client::handshake(tcp).await.unwrap();

    // Магия crate h2:
    tokio::spawn(async move {
        if let Err(e) = h2.await {
            println!("GOT ERR={:?}", e);
        }
    });

    // Я упакую отправленные запросы в вектор:
    let mut join_client_response_statistics = Vec::with_capacity(opt.number as usize);

    // Создаеи новые h2 соединения с сервером и отправляем запросы:
    for i in 0..opt.number {
        let mut clone_sendr = sender.clone();

        //  Заголовок запроса:
        let request = Request::builder()
            .uri(format!("https://http2.akamai.com/{}", i))
            .body(())
            .unwrap();

        // Отправляем запрос:
        if let Ok((response, _)) = clone_sendr.send_request(request, true) {
            let time = Instant::now();
            // Каждый запрос и обработка ответа асинхронны:
            let join = tokio::spawn(async move {
                let res = async {
                    // Дожидаемся ответа:
                    let _response = response.await?;

                    //println!("№ {} {}", i, response.status());
                    std::result::Result::<(), Box<dyn std::error::Error>>::Ok(())
                }
                .await;

                (time.elapsed(), res.is_ok())
            });

            join_client_response_statistics.push(join);
        } else {
            eprintln!("Ошибка отправки запроса серверу!");
            break;
        };
    }

    // Длинный блок кода, отвечающий ожидание завершения обработки ответов и вывод статистики. Он частично скопирован с кода сервера, это не хорошо, но оно работает.
    {
        let mut client_response_statistics = Vec::with_capacity(opt.number as usize);

        // Ждем получения ответов на все запросы:
        for join in join_client_response_statistics {
            let time = join.await.unwrap();
            client_response_statistics.push(time);
        }
        let responce_number = client_response_statistics.len();

        // Обрабатываем статистику:
        let goot_req = client_response_statistics
            .iter()
            .filter(|(_time, ok)| *ok)
            .count();

        // Посчитаем минимальное время ответа:
        let min = client_response_statistics
            .iter()
            //.filter(|(_time, err)| *err)
            .map(|(time, _ok)| time)
            .min()
            .unwrap_or(&Duration::ZERO);

        // Максимальное время ответа:
        let max = client_response_statistics
            .iter()
            //.filter(|(_time, err)| *err)
            .map(|(time, _ok)| time)
            .max()
            .unwrap_or(&Duration::ZERO);

        // Среднее арифметическое время ответа, можно было бы посчитать еще медиану:
        let sum: Duration = client_response_statistics
            .iter()
            //.filter(|(_time, err)| *err)
            .map(|(time, _ok)| time)
            .sum();

        let average = sum / responce_number as u32;

        println!(
            "Работа клиента завершена!
    Отправлено сообщений:               {}
    Количество сообщений с ответами:    {}
    Время сеанса клиента с сервером:    {} мс
    Минимальное время ответа:           {} мс
    Максимальное время ответа:          {} мс
    Среднее время ответа:               {} мс
    Общее время ответа всех запросов:   {} мс
    ",
            responce_number,
            goot_req,
            client_working_time.elapsed().as_millis(),
            min.as_millis(),
            max.as_millis(),
            average.as_millis(),
            sum.as_millis()
        )
    }
}
