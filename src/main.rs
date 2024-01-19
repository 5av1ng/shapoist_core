use shapoist_core::system::core_structs::ShapoistCore;

#[tokio::main]
async fn main() {
	env_logger::builder()
		.parse_default_env()
		.init();
	let mut core = ShapoistCore::new("./").unwrap();
	core.run_terminal_mode().await.unwrap();
}