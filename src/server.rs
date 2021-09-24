// Экспортируем StructOpt в server модуль, без него не сможем определить ServerOpt:
use crate::StructOpt;

// Собственно наш ServerOpt. Мы можем добовлять необходимые нам аргументы не затрагивая main код:
#[derive(Debug, StructOpt)]
pub struct ServerOpt {}

// Весь код, связанный с запуском сервера, пишем тут:
pub async fn run(_opt: ServerOpt) -> crate::Result<()> {
    Ok(())
}
