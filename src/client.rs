// Экспортируем StructOpt в server модуль, без него не сможем определить ServerOpt:
use crate::StructOpt;

// Собственно наш ClientOpt. Мы можем добовлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt)]
pub struct ClientOpt {}

// Весь код, связанный с запуском клиента, пишем тут:
pub async fn run(opt: ClientOpt) -> crate::Result<()> {
    dbg!(opt);
    Ok(())
}
