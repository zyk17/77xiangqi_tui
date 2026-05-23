mod analysis;
mod autoplay;
mod book_async;
mod command;
mod engine;
mod engine_policy;
mod game;

pub use analysis::AnalysisService;
pub use autoplay::{
    AI_MOVE_DELAY, AiPhase, AutoplayService, BOOK_ARROW_DELAY, ai_enabled_for_side,
    best_uci_from_book, best_uci_from_engine, should_query_book_for_display,
    should_try_book_for_autoplay,
};
pub use book_async::{BookQueryKind, BookQueryRuntime};
pub use command::{CommandService, CoordinateMove, ParsedCommand, SlashCommand};
pub use engine::EngineService;
pub use engine_policy::wants_shared_infinite_stream;
pub use game::GameService;

#[derive(Debug, Default)]
pub struct AppServices {
    pub command: CommandService,
    pub analysis: AnalysisService,
    pub book_queries: BookQueryRuntime,
    pub engine: EngineService,
}
