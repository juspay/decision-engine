use crate::error::ApiError;

///
/// Minimum limit of a card number will not deceed 8 by ISO standards
///
pub const MIN_CARD_NUMBER_LENGTH: usize = 8;

///
/// Maximum limit of a card number will not exceed 19 by ISO standards
///
pub const MAX_CARD_NUMBER_LENGTH: usize = 19;

pub fn sanitize_card_number(card_number: &str) -> Result<bool, ApiError> {
    let card_number = card_number.split_whitespace().collect::<String>();

    let is_card_number_valid = Ok(card_number.as_str())
        .and_then(validate_card_number_chars)
        .and_then(validate_card_number_length)
        .map(|number| luhn(&number))?;

    Ok(is_card_number_valid)
}

///
/// # Panics
///
/// Never, as a single character will never be greater than 10, or `u8`
///
pub fn validate_card_number_chars(number: &str) -> Result<Vec<u8>, ApiError> {
    let data = number.chars().try_fold(
        Vec::with_capacity(MAX_CARD_NUMBER_LENGTH),
        |mut data, character| {
            data.push(
                #[allow(clippy::expect_used)]
                character
                    .to_digit(10)
                    .ok_or(ApiError::ValidationError(
                        "invalid character found in card number",
                    ))?
                    .try_into()
                    .expect("error while converting a single character to u8"), // safety, a single character will never be greater `u8`
            );
            Ok::<Vec<u8>, ApiError>(data)
        },
    )?;

    Ok(data)
}

pub fn validate_card_number_length(number: Vec<u8>) -> Result<Vec<u8>, ApiError> {
    if number.len() >= MIN_CARD_NUMBER_LENGTH && number.len() <= MAX_CARD_NUMBER_LENGTH {
        Ok(number)
    } else {
        Err(ApiError::ValidationError("invalid card number length"))
    }
}

#[allow(clippy::as_conversions)]
pub fn luhn(number: &[u8]) -> bool {
    number
        .iter()
        .rev()
        .enumerate()
        .map(|(idx, element)| {
            ((*element * 2) / 10 + (*element * 2) % 10) * ((idx as u8) % 2)
                + (*element) * (((idx + 1) as u8) % 2)
        })
        .sum::<u8>()
        % 10
        == 0
}
