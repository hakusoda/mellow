use tokio::sync::OnceCell;

pub static STATE: OnceCell<State> = OnceCell::const_new();

#[derive(Debug)]
pub struct State {
	pub pg_pool: sqlx::PgPool
}