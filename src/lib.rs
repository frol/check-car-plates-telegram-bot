#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CarInfo {
    pub reported_in_city: Option<String>,
    pub car_brand: Option<String>,
    pub car_color: Option<String>,
    pub comment: Option<String>,
    pub number_of_people: Option<u8>,
}

impl std::fmt::Display for CarInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Авто: {car_brand}\nКолір авто: {car_color}\nОсобливості: {comment}\nЧисельність ДРГ: {number_of_people}\nМісто де вперше було зафіксовано: {car_reported_in_city}",
            car_reported_in_city=self.reported_in_city.as_deref().unwrap_or("?"),
            car_brand=self.car_brand.as_deref().unwrap_or("?"),
            car_color=self.car_color.as_deref().unwrap_or("?"),
            comment=self.comment.as_deref().unwrap_or("?"),
            number_of_people=self.number_of_people.as_ref().map(|x| x.to_string()).unwrap_or_else(|| "?".to_owned())
        )
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PartialLicensePlatesCarInfo {
    pub partial_license_plate: String,
    pub car_info: CarInfo,
}

impl PartialLicensePlatesCarInfo {
    pub fn matches(&self, license_plate: &str) -> bool {
        license_plate.contains(&self.partial_license_plate)
    }
}

/// 1. Uppercase
/// 2. Replace all letter that could look similarly in different languages to latin variant (convert A in cyrylic to A in latin)
/// 3. Clean up from non-letter/non-digit characters (e.g. hyphen, spaces)
///
/// ```
/// use check_car_plates_telegram_bot::normalize_license_plate;
/// assert_eq!(normalize_license_plate("вт 12-34 см"), "BT1234CM");
/// assert_eq!(normalize_license_plate("вт 12-34 cm"), "BT1234CM");
/// ```
pub fn normalize_license_plate(raw_license_plate: &str) -> String {
    raw_license_plate
        .to_uppercase()
        .replace('А', "A")
        .replace('В', "B")
        .replace('Е', "E")
        .replace('К', "K")
        .replace('М', "M")
        .replace('І', "I")
        .replace('Н', "H")
        .replace('О', "O")
        .replace('Р', "P")
        .replace('С', "C")
        .replace('Т', "T")
        .replace('У', "Y")
        .replace('Х', "X")
        .replace(|ch: char| !ch.is_alphanumeric(), "")
}
