// Я решил вынесть серверную и клиентскую чать кода в отдельные модули. Подумал что так код станет выглядить чище:
mod client;
mod server;

// Объявим crates, которые я буду использовать в этом тэстовом приложении:
use {anyhow::Result, structopt::StructOpt, tokio};

// Определим структуру с trait StructOpt, она поможет нам понять с какой ролью мы запустили наше приложение:
#[derive(Debug, StructOpt)]
#[structopt(about = "Тестовое Client/Server приложение")]
enum Opt {
    Server(ServerOpt),
    Client(ClientOpt),
}

// Для удобства передачи cfg клиента и сервера определим две структуры:
#[derive(Debug, StructOpt)]
pub struct ServerOpt {}

#[derive(Debug, StructOpt)]
pub struct ClientOpt {}

// Оприделим функцию main с использованиям макроса #[toki::main], main добжна быть async:
#[tokio::main]
async fn main() -> Result<()> {
    match Opt::from_args() {
        Opt::Server(opt) => server::run(opt).await,
        Opt::Client(opt) => client::run(opt).await,
    }
}
