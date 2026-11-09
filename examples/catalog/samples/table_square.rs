//! Square table: the same working table with 0px corners. Sorting,
//! selection and scrolling behave exactly as they do with rounded corners.
#[cfg(feature = "native")] // catalog-only
use volt_ui_kit::native::Application;
use volt_ui_kit::ui::prelude::*;

/// Invoice, status, payment method, amount.
const INVOICES: [[&str; 4]; 6] = [
    ["INV001", "Paid", "Credit card", "$250.00"],
    ["INV002", "Pending", "PayPal", "$150.00"],
    ["INV003", "Unpaid", "Bank transfer", "$350.00"],
    ["INV004", "Paid", "Credit card", "$450.00"],
    ["INV005", "Paid", "PayPal", "$550.00"],
    ["INV006", "Pending", "Bank transfer", "$200.00"],
];

#[derive(Default)]
pub struct App {
    /// The selected row: its index into `INVOICES`.
    pub row: Option<u32>,
    /// The sorted column and direction, once a header is clicked.
    pub sort: Option<(u8, SortDirection)>,
}

pub fn view(app: &App) -> Element<App> {
    let columns = [
        table_column(0u8, "Invoice").width(130.),
        table_column(1, "Status").width(120.),
        table_column(2, "Method").width(160.).sortable(false),
        table_column(3, "Amount").width(110.).align(Align::End),
    ];
    let rows = INVOICES
        .iter()
        .enumerate()
        .map(|(i, cells)| table_row(i as u32, *cells).key(format!("inv-square-{i}")));
    let mut invoices = table(columns, rows)
        .key("invoices-square")
        .width(600.)
        .rounded(0.) // square outer frame and inset rows
        .accessible_label("Square invoices")
        .selected(app.row)
        .on_select(|app: &mut App, row| app.row = Some(row))
        .on_sort(|app: &mut App, column, direction| app.sort = Some((column, direction)));
    if let Some((column, direction)) = app.sort {
        invoices = invoices.sort(column, direction);
    }
    let status = match app.row {
        Some(row) => format!("{} invoices · {} selected", INVOICES.len(), INVOICES[row as usize][0]),
        None => format!("{} invoices", INVOICES.len()),
    };
    vstack![
        invoices,
        text(status).font_size(12.).color(ColorToken::Muted),
    ]
    .gap(12., Axis::Vertical)
    .into()
}

#[cfg(feature = "native")] // catalog-only
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Square invoices", App::default(), view)
        .size(680, 420)
        .run()
}
