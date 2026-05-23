mod analysis;
mod autoplay;
mod book;
mod command;
mod engine;
mod game;

pub use analysis::AnalysisService;
pub use autoplay::{
    AI_MOVE_DELAY, AiPhase, AutoplayService, BOOK_ARROW_DELAY, ai_enabled_for_side,
    best_uci_from_engine, book_config_usable, should_query_book_for_display,
};
pub use book::BookService;
pub use command::{CommandService, CoordinateMove, ParsedCommand, SlashCommand};
pub use engine::EngineService;
pub use game::GameService;

#[derive(Debug, Default)]
pub struct AppServices {
    pub command: CommandService,
    pub analysis: AnalysisService,
    pub book: BookService,
    pub engine: EngineService,
}
