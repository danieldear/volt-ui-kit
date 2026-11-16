//! Invoice table recipes. Run with `--square` for the square-corner variant.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
pub struct App {
    pub row: Option<u32>,
    pub sort: Option<(u8, SortDirection)>,
}

pub fn view(app: &App) -> Element<App> {
    invoice_table(app, false)
}
pub fn square_view(app: &App) -> Element<App> {
    invoice_table(app, true)
}

pub const INVOICES: [(&str, &str, &str, &str); 6] = [
    ("INV001", "Paid", "Credit card", "$250.00"),
    ("INV002", "Pending", "PayPal", "$150.00"),
    ("INV003", "Unpaid", "Bank transfer", "$350.00"),
    ("INV004", "Paid", "Credit card", "$450.00"),
    ("INV005", "Paid", "PayPal", "$550.00"),
    ("INV006", "Pending", "Bank transfer", "$200.00"),
];
fn invoice_table(app: &App, square: bool) -> Element<App> {
    let key = if square {
        "invoices-square"
    } else {
        "invoices"
    };
    let row_prefix = if square { "inv-square" } else { "inv" };
    let mut t = table(
        [
            table_column(0u8, "Invoice").width(130.).sortable(true),
            table_column(1, "Status").width(120.).sortable(true),
            table_column(2, "Method").width(160.).sortable(false),
            table_column(3, "Amount")
                .width(110.)
                .align(Align::End)
                .sortable(true),
        ],
        INVOICES.iter().enumerate().map(|(i, r)| {
            table_row(i as u32, [r.0, r.1, r.2, r.3]).key(format!("{row_prefix}-{i}"))
        }),
    )
    .key(key)
    .width(600.)
    .rounded(if square { 0. } else { 12. })
    .accessible_label(if square {
        "Square invoices"
    } else {
        "Rounded invoices"
    })
    .selected(app.row)
    .on_select(|app: &mut App, r| app.row = Some(r))
    .on_sort(|app: &mut App, column, direction| app.sort = Some((column, direction)));
    if let Some((column, direction)) = app.sort {
        t = t.sort(column, direction);
    }
    t.into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let square = std::env::args()
        .any(|arg| matches!(arg.as_str(), "--square" | "table-square" | "table_square"));
    Application::new("Invoices", App::default(), move |app| {
        let demo = if square { square_view(app) } else { view(app) };
        vstack![demo]
            .align(Align::Center)
            .padding(24., Edge::All)
            .fill_width()
            .fill_height()
            .into()
    })
    .size(680, 440)
    .run()
}
