// Я решил вынесть серверную и клиентскую чать кода в отдельные модули. Подумал что так код станет выглядить чище:
mod client;
mod server;

// Объявим crates, которые я буду использовать в этом тэстовом приложении:
use {anyhow::Result, structopt::StructOpt, tokio};

// Определим структуру с trait StructOpt, она поможет нам понять с какой ролью мы запустили наше приложение:
#[derive(StructOpt, Debug)]
#[structopt(about = "Тестовое Client/Server приложение")]
enum Opt {
    Server {},
    Client {},
}

// Оприделим функцию main с использованиям макроса #[toki::main], main добжна быть async:
#[tokio::main]
async fn main() -> Result<()> {
    match Opt::from_args() {
        Opt::Server {} => server::run().await,
        Opt::Client {} => client::run().await,
    }
}
