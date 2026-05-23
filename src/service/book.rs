use crate::book::{BookConfig, BookResponse, query_opening_book};

#[derive(Debug, Clone)]
pub struct BookQuery<'a> {
    pub fen: &'a str,
    pub move_uci: Option<String>,
    pub ignore_play_opening_settings: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BookService;

impl BookService {
    pub fn query(&self, query: BookQuery<'_>, config: &BookConfig) -> BookResponse {
        query_opening_book(
            query.fen,
            query.move_uci,
            config,
            query.ignore_play_opening_settings,
        )
    }
}
