use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

const MODULE_LOG_FILTERS: &str = concat!(
  "ERROR,",
  "mdbook-epub=ERROR,",
  "epub_builder=ERROR,",
  "handlebars=ERROR,",
  "mdbook_core=ERROR,",
  "mdbook_renderer=ERROR,",
  "pulldown_cmark=ERROR,",
  "ureq=ERROR,",
  "ureq_proto=ERROR",
);

pub fn init_tracing() {
  let fmt_layer = fmt::layer()
    .with_level(true) // Показываем уровень логирования
    .with_ansi(true) // Включаем цвет (для читаемости)
    .event_format(tracing_subscriber::fmt::format().compact()) // Компактный формат логов
    .compact();

  let env_filter = match std::env::var("RUST_LOG") {
    // можно передать свой набор фильтров через переменную окружения
    Ok(_) => EnvFilter::from_env("RUST_LOG"),
    // если отсутствует RUST_LOG, то по умолчанию загрузит MODULE_LOG_FILTERS
    Err(_) => EnvFilter::new(MODULE_LOG_FILTERS),
  };

  tracing_subscriber::registry().with(fmt_layer).with(env_filter).init();
}
