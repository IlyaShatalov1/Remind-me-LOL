use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup};

// Creates a keyboard made by buttons in a big column.
pub fn make_inline_keyboard(
    vector_names: Vec<&str>,
    vector_data: Vec<&str>,
    chunks: usize,
) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    // 1. Собираем пары (имя, данные) в промежуточный вектор
    let combined: Vec<_> = vector_names.iter().zip(vector_data.iter()).collect();

    // 2. Итерируемся по кускам (строкам клавиатуры)
    for chunk in combined.chunks(chunks) {
        let row = chunk
            .iter()
            .map(|(name, data)| {
                // 3. Создаем кнопку (клонируем строки для callback_data)
                InlineKeyboardButton::callback(name.to_string(), data.to_string())
            })
            .collect::<Vec<InlineKeyboardButton>>();

        // 4. Добавляем строку в общую разметку
        keyboard.push(row);
    }

    InlineKeyboardMarkup::new(keyboard)
}

pub fn make_keyboard(vector: Vec<String>, chunks: usize) -> KeyboardMarkup {
    let mut keyboard: Vec<Vec<KeyboardButton>> = vec![];

    for elements in vector.chunks(chunks) {
        let row = elements.iter().map(KeyboardButton::new).collect();

        keyboard.push(row);
    }

    KeyboardMarkup::new(keyboard).resize_keyboard()
    //.one_time_keyboard()
}
