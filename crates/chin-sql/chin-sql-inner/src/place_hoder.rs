pub enum PlaceHolderType {
    QustionMark,
    DollarNumber(i32),
}

impl PlaceHolderType {
    pub fn dollar_number() -> Self {
        PlaceHolderType::DollarNumber(0)
    }

    pub fn question_mark() -> Self {
        PlaceHolderType::QustionMark
    }

    pub fn next_ph(&mut self) -> String {
        match self {
            PlaceHolderType::QustionMark => "?".to_owned(),
            PlaceHolderType::DollarNumber(n) => {
                *n += 1;
                format!("${n}")
            }
        }
    }
}
