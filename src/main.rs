// Объявим crates, которые я буду использовать в этом тэстовом приложении:
use structopt::StructOpt;

// Определим структуру с trait StructOpt, она поможет нам понять с какой ролью мы запустили наше приложение:
#[derive(StructOpt, Debug)]
#[structopt(about = "Тестовое Client/Server приложение")]
enum Opt {
    Server {},
    Client {},
}
fn main() {
    dbg!(Opt::from_args());
}
