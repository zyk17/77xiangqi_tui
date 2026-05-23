mod analysis;
mod autoplay;
mod book;
mod command;
mod engine;
mod game;

pub use analysis::AnalysisService;
pub use autoplay::{
    ai_enabled_for_side, book_config_usable, best_uci_from_engine, should_query_book_for_display,
    AiPhase, AutoplayService, AI_MOVE_DELAY, BOOK_ARROW_DELAY,
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
    pub game: GameService,
}
