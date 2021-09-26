// Я решил вынесть серверную и клиентскую чать кода в отдельные модули. Подумал что так код станет выглядить чище:
mod client;
mod server;

// Объявим crates, которые я буду использовать в этом тэстовом приложении:
use structopt::StructOpt;

// Определим структуру с trait StructOpt, она поможет нам понять с какой ролью мы запустили наше приложение:
#[derive(Debug, StructOpt)]
#[structopt(about = "Тестовое Client/Server приложение")]
enum Opt {
    Server(server::ServerOpt),
    Client(client::ClientOpt),
}

// Оприделим функцию main с использованиям макроса #[toki::main], main добжна быть async:
#[tokio::main]
async fn main() {
    match Opt::from_args() {
        Opt::Server(opt) => server::run(opt).await,
        Opt::Client(opt) => client::run(opt).await,
    };
}
