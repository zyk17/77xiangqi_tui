mod analysis;
mod book;
mod command;
mod engine;

pub use analysis::AnalysisService;
pub use book::BookService;
pub use command::{CommandService, CoordinateMove, ParsedCommand, SlashCommand};
pub use engine::EngineService;

#[derive(Debug, Default)]
pub struct AppServices {
    pub command: CommandService,
    pub analysis: AnalysisService,
    pub book: BookService,
    pub engine: EngineService,
}
