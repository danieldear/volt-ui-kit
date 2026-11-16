//! Pantry: a branded grocery shop. Product grid, product page, cart and
//! checkout, drawn by Volt in the shop's own colours; navigation, scrolling,
//! switches and the keyboard stay native.
use super::*;

/// The shop's brand colour and its deal highlight.
pub const BRAND: &str = "#0E7C66";
pub const BRAND_RGBA: [f32; 4] = [0.055, 0.486, 0.4, 1.];
const DEAL: &str = "#FFC93C";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Category {
    All,
    Produce,
    Bakery,
    Dairy,
    Drinks,
}
impl Category {
    const ALL: [Category; 5] = [
        Category::All,
        Category::Produce,
        Category::Bakery,
        Category::Dairy,
        Category::Drinks,
    ];
    fn label(self) -> &'static str {
        match self {
            Category::All => "All",
            Category::Produce => "Produce",
            Category::Bakery => "Bakery",
            Category::Dairy => "Dairy",
            Category::Drinks => "Drinks",
        }
    }
}

/// How a product's picture is drawn until the shop has photos.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Fruit(&'static str),
    Loaf,
    Bottle(&'static str),
    Carton,
    Wedge,
    Cup(&'static str),
}

pub struct Product {
    pub id: u8,
    pub name: &'static str,
    size: &'static str,
    pub cents: u32,
    /// The price before a deal.
    was: Option<u32>,
    category: Category,
    tile: &'static str,
    shape: Shape,
    rating: f32,
    reviews: u32,
    about: &'static str,
}

pub const PRODUCTS: [Product; 10] = [
    Product {
        id: 1,
        name: "Honeycrisp apples",
        size: "1 kg bag",
        cents: 449,
        was: Some(549),
        category: Category::Produce,
        tile: "#FDE8E4",
        shape: Shape::Fruit("#E5484D"),
        rating: 4.8,
        reviews: 1204,
        about: "Crisp and sweet, picked this week. Great for snacking and baking.",
    },
    Product {
        id: 2,
        name: "Navel oranges",
        size: "6 pack",
        cents: 399,
        was: None,
        category: Category::Produce,
        tile: "#FFF1DD",
        shape: Shape::Fruit("#F5A623"),
        rating: 4.6,
        reviews: 642,
        about: "Seedless and juicy. Six large oranges.",
    },
    Product {
        id: 3,
        name: "Limes",
        size: "500 g",
        cents: 249,
        was: None,
        category: Category::Produce,
        tile: "#E8F6E1",
        shape: Shape::Fruit("#6BBF59"),
        rating: 4.5,
        reviews: 318,
        about: "Bright and tart, for drinks, dressings and tacos.",
    },
    Product {
        id: 4,
        name: "Sourdough loaf",
        size: "800 g",
        cents: 549,
        was: Some(649),
        category: Category::Bakery,
        tile: "#F7EBDD",
        shape: Shape::Loaf,
        rating: 4.9,
        reviews: 2210,
        about: "Baked this morning with a slow-rise starter. Crackly crust.",
    },
    Product {
        id: 5,
        name: "Whole milk",
        size: "2 l",
        cents: 289,
        was: None,
        category: Category::Dairy,
        tile: "#E8F1FB",
        shape: Shape::Carton,
        rating: 4.7,
        reviews: 980,
        about: "Fresh whole milk from local farms.",
    },
    Product {
        id: 6,
        name: "Aged cheddar",
        size: "250 g",
        cents: 629,
        was: None,
        category: Category::Dairy,
        tile: "#FFF6D6",
        shape: Shape::Wedge,
        rating: 4.8,
        reviews: 455,
        about: "Matured for 18 months. Sharp and crumbly.",
    },
    Product {
        id: 7,
        name: "Greek yogurt",
        size: "500 g",
        cents: 359,
        was: Some(429),
        category: Category::Dairy,
        tile: "#EEF0F5",
        shape: Shape::Cup("#7C8CF8"),
        rating: 4.6,
        reviews: 731,
        about: "Thick and creamy, with live cultures.",
    },
    Product {
        id: 8,
        name: "Orange juice",
        size: "1 l",
        cents: 429,
        was: None,
        category: Category::Drinks,
        tile: "#FFF1DD",
        shape: Shape::Bottle("#F5A623"),
        rating: 4.4,
        reviews: 512,
        about: "Pressed oranges and nothing else.",
    },
    Product {
        id: 9,
        name: "Sparkling water",
        size: "6 × 330 ml",
        cents: 499,
        was: Some(599),
        category: Category::Drinks,
        tile: "#E3F4F4",
        shape: Shape::Bottle("#3BA7A0"),
        rating: 4.7,
        reviews: 1630,
        about: "Lightly sparkling, with a hint of lime.",
    },
    Product {
        id: 10,
        name: "Croissants",
        size: "4 pack",
        cents: 479,
        was: None,
        category: Category::Bakery,
        tile: "#FBEBD8",
        shape: Shape::Loaf,
        rating: 4.8,
        reviews: 870,
        about: "Laminated with butter and baked golden.",
    },
];

pub fn product(id: u8) -> &'static Product {
    PRODUCTS.iter().find(|p| p.id == id).unwrap_or(&PRODUCTS[0])
}
pub fn price(cents: u32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}
pub fn cart_count(app: &App) -> u32 {
    app.cart.values().sum()
}
fn subtotal(app: &App) -> u32 {
    app.cart
        .iter()
        .map(|(id, qty)| product(*id).cents * qty)
        .sum()
}
fn delivery(app: &App) -> u32 {
    if subtotal(app) >= 3500 {
        0
    } else {
        399
    }
}
fn tip(app: &App) -> u32 {
    subtotal(app) * app.tip / 100
}
fn total(app: &App) -> u32 {
    subtotal(app) + delivery(app) + 199 + tip(app)
}
fn add(app: &mut App, id: u8, qty: u32) {
    *app.cart.entry(id).or_insert(0) += qty;
}
fn remove_one(app: &mut App, id: u8) {
    if let Some(qty) = app.cart.get_mut(&id) {
        *qty -= 1;
        if *qty == 0 {
            app.cart.remove(&id);
        }
    }
}

/// A product picture: a soft tile with the product drawn in its middle.
fn picture(p: &Product, height: f32, art: f32, key: String) -> Element<App> {
    let shape = p.shape;
    let s = art / 100.;
    let drawing = canvas(move |c| {
        let v = |x: f32| x * s;
        let leaf = "#3A9D5D";
        match shape {
            Shape::Fruit(color) => {
                c.fill(VectorPath::circle(v(50.), v(58.), v(30.)), color);
                c.fill(
                    VectorPath::ellipse(v(40.), v(48.), v(8.), v(5.)),
                    "#FFFFFF55",
                );
                c.fill(
                    VectorPath::rectangle(v(48.), v(20.), v(4.), v(12.)),
                    "#7A5230",
                );
                c.fill(
                    VectorPath::new()
                        .move_to(v(52.), v(26.))
                        .quad_to([v(66.), v(12.)], [v(76.), v(20.)])
                        .quad_to([v(66.), v(32.)], [v(52.), v(26.)])
                        .close(),
                    leaf,
                );
            }
            Shape::Loaf => {
                c.fill(
                    VectorPath::ellipse(v(50.), v(50.), v(36.), v(22.)),
                    "#C68642",
                );
                c.fill(
                    VectorPath::rectangle(v(14.), v(50.), v(72.), v(20.)),
                    "#C68642",
                );
                for x in [34., 50., 66.] {
                    c.stroke(
                        VectorPath::new()
                            .move_to(v(x - 6.), v(46.))
                            .line_to(v(x + 6.), v(36.)),
                        "#E8B377",
                        v(4.),
                    );
                }
            }
            Shape::Bottle(color) => {
                c.fill(
                    VectorPath::rectangle(v(42.), v(14.), v(16.), v(10.)),
                    "#2F3A45",
                );
                c.fill(VectorPath::rectangle(v(38.), v(24.), v(24.), v(10.)), color);
                c.fill(VectorPath::rectangle(v(32.), v(34.), v(36.), v(54.)), color);
                c.fill(
                    VectorPath::rectangle(v(32.), v(52.), v(36.), v(18.)),
                    "#FFFFFF",
                );
            }
            Shape::Carton => {
                c.fill(
                    VectorPath::new()
                        .move_to(v(30.), v(34.))
                        .line_to(v(50.), v(16.))
                        .line_to(v(70.), v(34.))
                        .close(),
                    "#D9E6F5",
                );
                c.fill(
                    VectorPath::rectangle(v(30.), v(34.), v(40.), v(54.)),
                    "#FFFFFF",
                );
                c.fill(
                    VectorPath::rectangle(v(30.), v(50.), v(40.), v(16.)),
                    "#4A7FD4",
                );
            }
            Shape::Wedge => {
                c.fill(
                    VectorPath::new()
                        .move_to(v(16.), v(74.))
                        .line_to(v(84.), v(74.))
                        .line_to(v(74.), v(30.))
                        .close(),
                    "#F5C542",
                );
                for (x, y, r) in [(54., 62., 5.), (68., 52., 4.), (40., 68., 3.)] {
                    c.fill(VectorPath::circle(v(x), v(y), v(r)), "#D9A520");
                }
            }
            Shape::Cup(color) => {
                c.fill(VectorPath::rectangle(v(28.), v(30.), v(44.), v(8.)), color);
                c.fill(
                    VectorPath::new()
                        .move_to(v(30.), v(38.))
                        .line_to(v(70.), v(38.))
                        .line_to(v(64.), v(84.))
                        .line_to(v(36.), v(84.))
                        .close(),
                    "#FFFFFF",
                );
                c.fill(VectorPath::rectangle(v(34.), v(52.), v(32.), v(14.)), color);
            }
        }
    })
    .width(art)
    .height(art);
    zstack![drawing]
        .key(key)
        .width_fraction(1.)
        .height(height)
        .background(p.tile)
        .rounded(20.)
        .into()
}

/// A compact quantity control: minus, count, plus.
fn stepper(id: u8, qty: u32, key: &str) -> Element<App> {
    hstack![
        button("−")
            .key(format!("{key}-minus"))
            .accessible_label("Remove one")
            .variant(ButtonVariant::Ghost)
            .color(BRAND)
            .font_size(20.)
            .width(40.)
            .height(36.)
            .on_click(move |a: &mut App| remove_one(a, id)),
        text(qty.to_string())
            .key(format!("{key}-qty"))
            .font_size(17.)
            .font_weight(600),
        button("+")
            .key(format!("{key}-plus"))
            .accessible_label("Add one")
            .variant(ButtonVariant::Ghost)
            .color(BRAND)
            .font_size(20.)
            .width(40.)
            .height(36.)
            .on_click(move |a: &mut App| add(a, id, 1)),
    ]
    .gap(4., Axis::Horizontal)
    .align(Align::Center)
    .justify(Justify::SpaceBetween)
    .background(ColorToken::Base200)
    .rounded(18.)
    .into()
}

/// A product card: picture, name, size and price, with a quick add.
fn card(app: &App, p: &'static Product, key: &str, width: Option<f32>) -> Element<App> {
    let id = p.id;
    let qty = app.cart.get(&id).copied().unwrap_or(0);
    let action: Element<App> = if qty == 0 {
        button("+")
            .key(format!("{key}-add"))
            .accessible_label(format!("Add {}", p.name))
            .variant(ButtonVariant::Primary)
            .font_size(20.)
            .font_weight(600)
            .width(36.)
            .height(36.)
            .rounded(18.)
            .on_click(move |a: &mut App| add(a, id, 1))
            .into()
    } else {
        stepper(id, qty, key)
    };
    let mut price_row: Vec<Element<App>> =
        vec![text(price(p.cents)).font_size(17.).font_weight(700).into()];
    if let Some(was) = p.was {
        price_row.push(
            text(price(was))
                .font_size(13.)
                .color(ColorToken::Muted)
                .into(),
        );
    }
    let mut layers: Vec<Element<App>> = vec![picture(p, 132., 84., format!("{key}-picture"))];
    if p.was.is_some() {
        layers.push(
            hstack![badge("Deal")
                .background(DEAL)
                .color("#1A1A1A")
                .font_size(12.)]
            .padding(10., Edge::All)
            .into(),
        );
    }
    let price_row = hstack(price_row)
        .gap(6., Axis::Horizontal)
        .align(Align::Center);
    // In the cart: the stepper takes its own row so the price never wraps.
    let bottom: Element<App> = if qty == 0 {
        hstack![price_row, spacer(), action]
            .align(Align::Center)
            .into()
    } else {
        vstack![price_row, action]
            .gap(8., Axis::Vertical)
            .align(Align::Stretch)
            .into()
    };
    let mut c = vstack![
        zstack(layers).flow(Flow::TopLeft).width_fraction(1.),
        vstack![
            text(p.name).font_size(15.).font_weight(600),
            text(p.size).font_size(13.).color(ColorToken::Muted),
        ]
        .gap(2., Axis::Vertical),
        bottom,
    ]
    .key(key)
    .gap(8., Axis::Vertical)
    .align(Align::Stretch)
    .accessible_label(p.name)
    .on_click_with(move |a: &mut App, _| {
        a.quantity = 1;
        a.history.push(Route::Product(id));
    });
    if let Some(w) = width {
        c = c.width(w);
    } else {
        c = c.flex(1.);
    }
    c.into()
}

pub fn home(app: &App) -> Container<App> {
    let query = app.search.trim().to_lowercase();
    let shown: Vec<&'static Product> = PRODUCTS
        .iter()
        .filter(|p| app.category == Category::All || p.category == app.category)
        .filter(|p| query.is_empty() || p.name.to_lowercase().contains(&query))
        .collect();
    let chips = Category::ALL
        .iter()
        .map(|&c| {
            let on = app.category == c;
            button(c.label())
                .key(format!("chip-{}", c.label().to_lowercase()))
                .font_size(15.)
                .font_weight(600)
                .height(36.)
                .rounded(18.)
                .background(if on { BRAND } else { "#00000000" })
                .color(if on { "#FFFFFF" } else { BRAND })
                .border_color(BRAND)
                .border_width(1.)
                .on_click(move |a: &mut App| a.category = c)
                .into()
        })
        .collect::<Vec<Element<App>>>();
    let deals = PRODUCTS
        .iter()
        .filter(|p| p.was.is_some())
        .map(|p| card(app, p, &format!("deal-{}", p.id), Some(156.)))
        .collect::<Vec<_>>();
    let mut grid: Vec<Element<App>> = Vec::new();
    for pair in shown.chunks(2) {
        let mut row: Vec<Element<App>> = pair
            .iter()
            .map(|p| card(app, p, &format!("card-{}", p.id), None))
            .collect();
        if row.len() == 1 {
            row.push(spacer().into());
        }
        grid.push(hstack(row).gap(14., Axis::Horizontal).into());
    }
    if grid.is_empty() {
        grid.push(
            hstack![text(format!("No products match “{}”.", app.search.trim()))
                .font_size(15.)
                .color(ColorToken::Muted)]
            .padding(16., Edge::Vertical)
            .into(),
        );
    }
    let mut parts: Vec<Element<App>> = vec![
        hstack![
            icon(Icon::Home).size(18.).color(BRAND),
            vstack![
                text("Deliver to").font_size(13.).color(ColorToken::Muted),
                text("Home · 15–25 min").font_size(15.).font_weight(600),
            ],
            spacer(),
            icon(Icon::ChevronDown).size(16.).color(ColorToken::Muted),
        ]
        .key("address")
        .gap(10., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        hstack![
            icon(Icon::Search).size(18.).color(ColorToken::Muted),
            text_field(&app.search)
                .key("search")
                .accessible_label("Search Pantry")
                .placeholder("Search Pantry")
                .font_size(17.)
                .height(44.)
                .background("#00000000")
                .on_change(|a: &mut App, v| a.search = v)
                .flex(1.),
        ]
        .padding(12., Edge::Left)
        .gap(4., Axis::Horizontal)
        .align(Align::Center)
        .background(ColorToken::Base200)
        .rounded(22.)
        .into(),
        hstack(chips)
            .key("chips")
            .gap(8., Axis::Horizontal)
            .scroll(Scroll::Horizontal)
            .into(),
    ];
    if query.is_empty() && app.category == Category::All {
        parts.push(heading("Deals for you"));
        parts.push(
            hstack(deals)
                .key("deals")
                .gap(14., Axis::Horizontal)
                .scroll(Scroll::Horizontal)
                .into(),
        );
    }
    parts.push(heading(if app.category == Category::All {
        "Popular"
    } else {
        app.category.label()
    }));
    parts.push(vstack(grid).gap(22., Axis::Vertical).into());
    shop_page("shop-page", parts)
}
fn heading(title: &str) -> Element<App> {
    text(title).font_size(22.).font_weight(700).into()
}
fn shop_page(key: &str, parts: Vec<Element<App>>) -> Container<App> {
    vstack![vstack(parts)
        .align(Align::Stretch)
        .gap(18., Axis::Vertical)
        .padding(12., Edge::Top)
        .padding(32., Edge::Bottom)]
    .key(key)
    .padding(16., Edge::Horizontal)
    .background(ColorToken::Base100)
    .flex(1.)
    .scroll(Scroll::Vertical)
}

pub fn product_page(app: &App, id: u8) -> Container<App> {
    let p = product(id);
    let qty = app.quantity.max(1);
    let full = (p.rating.round() as usize).min(5);
    let mut stars: Vec<Element<App>> = (0..5)
        .map(|i| {
            icon(if i < full {
                Icon::StarFilled
            } else {
                Icon::Star
            })
            .size(16.)
            .color(DEAL)
            .into()
        })
        .collect();
    stars.push(
        text(format!("{:.1} · {} reviews", p.rating, p.reviews))
            .font_size(15.)
            .color(ColorToken::Muted)
            .into(),
    );
    let mut price_row: Vec<Element<App>> =
        vec![text(price(p.cents)).font_size(28.).font_weight(700).into()];
    if let Some(was) = p.was {
        price_row.push(
            text(format!("was {}", price(was)))
                .font_size(15.)
                .color(ColorToken::Muted)
                .into(),
        );
        price_row.push(
            badge(format!("Save {}", price(was - p.cents)))
                .background(DEAL)
                .color("#1A1A1A")
                .font_size(12.)
                .into(),
        );
    }
    shop_page(
        &format!("product-{id}"),
        vec![
            picture(p, 300., 200., format!("hero-{id}")),
            vstack![
                text(p.name).font_size(28.).font_weight(700),
                text(p.size).font_size(17.).color(ColorToken::Muted),
            ]
            .gap(4., Axis::Vertical)
            .into(),
            hstack(stars)
                .gap(4., Axis::Horizontal)
                .align(Align::Center)
                .into(),
            hstack(price_row)
                .gap(10., Axis::Horizontal)
                .align(Align::Center)
                .into(),
            text(p.about).font_size(17.).into(),
            hstack![
                hstack![
                    button("−")
                        .key("qty-minus")
                        .accessible_label("Fewer")
                        .variant(ButtonVariant::Ghost)
                        .color(BRAND)
                        .font_size(22.)
                        .width(48.)
                        .height(52.)
                        .enabled(qty > 1)
                        .on_click(|a: &mut App| a.quantity = a.quantity.max(1) - 1),
                    text(qty.to_string())
                        .key("qty")
                        .font_size(20.)
                        .font_weight(600),
                    button("+")
                        .key("qty-plus")
                        .accessible_label("More")
                        .variant(ButtonVariant::Ghost)
                        .color(BRAND)
                        .font_size(22.)
                        .width(48.)
                        .height(52.)
                        .on_click(|a: &mut App| a.quantity = a.quantity.max(1) + 1),
                ]
                .align(Align::Center)
                .background(ColorToken::Base200)
                .rounded(26.),
                button(format!("Add {qty} · {}", price(p.cents * qty)))
                    .key("add-to-cart")
                    .variant(ButtonVariant::Primary)
                    .font_size(17.)
                    .font_weight(600)
                    .height(52.)
                    .rounded(26.)
                    .flex(1.)
                    .on_click(move |a: &mut App| {
                        add(a, id, a.quantity.max(1));
                        a.quantity = 1;
                        a.history.back();
                    }),
            ]
            .gap(12., Axis::Horizontal)
            .into(),
        ],
    )
}

pub fn cart(app: &App) -> Container<App> {
    if app.cart.is_empty() {
        return shop_page(
            "cart-page",
            vec![vstack![
                icon(Icon::Inbox).size(48.).color(ColorToken::Muted),
                text("Your cart is empty").font_size(22.).font_weight(700),
                text("Add something from the shop and it shows up here.")
                    .font_size(15.)
                    .color(ColorToken::Muted),
                button("Start shopping")
                    .key("start-shopping")
                    .variant(ButtonVariant::Primary)
                    .font_size(17.)
                    .font_weight(600)
                    .height(52.)
                    .rounded(26.)
                    .on_click(|a: &mut App| {
                        a.history.back();
                    }),
            ]
            .gap(12., Axis::Vertical)
            .align(Align::Center)
            .padding(60., Edge::Top)
            .into()],
        );
    }
    let rows = app
        .cart
        .iter()
        .map(|(&id, &qty)| {
            let p = product(id);
            hstack![
                picture(p, 64., 48., format!("line-{id}-picture")).width(64.),
                vstack![
                    text(p.name).font_size(15.).font_weight(600),
                    text(price(p.cents * qty))
                        .font_size(15.)
                        .color(ColorToken::Muted),
                ]
                .gap(2., Axis::Vertical)
                .flex(1.),
                stepper(id, qty, &format!("line-{id}")),
            ]
            .key(format!("line-{id}"))
            .gap(12., Axis::Horizontal)
            .align(Align::Center)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    let line = |label: &str, value: String, key: &str| -> Element<App> {
        hstack![
            text(label).font_size(15.).color(ColorToken::Muted),
            spacer(),
            text(value).key(key.to_owned()).font_size(15.),
        ]
        .into()
    };
    let free = 3500u32.saturating_sub(subtotal(app));
    shop_page(
        "cart-page",
        vec![
            vstack(rows).gap(14., Axis::Vertical).into(),
            divider().height(1. / 3.).into(),
            vstack![
                line("Subtotal", price(subtotal(app)), "subtotal"),
                line(
                    "Delivery",
                    if delivery(app) == 0 {
                        "Free".into()
                    } else {
                        price(delivery(app))
                    },
                    "delivery"
                ),
                line("Service fee", price(199), "service"),
            ]
            .gap(8., Axis::Vertical)
            .into(),
            if free > 0 {
                text(format!("Add {} more for free delivery.", price(free)))
                    .font_size(13.)
                    .color(BRAND)
                    .into()
            } else {
                text("You get free delivery.")
                    .font_size(13.)
                    .color(BRAND)
                    .into()
            },
            button(format!("Checkout · {}", price(total(app) - tip(app))))
                .key("checkout")
                .variant(ButtonVariant::Primary)
                .font_size(17.)
                .font_weight(600)
                .height(52.)
                .rounded(26.)
                .on_click(|a: &mut App| {
                    a.history.push(Route::Checkout);
                })
                .into(),
        ],
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Window {
    Now,
    Tonight,
    Tomorrow,
}

pub fn checkout(app: &App) -> Container<App> {
    let row = |label: &str, value: &str| -> Element<App> {
        hstack![
            text(label).font_size(17.),
            spacer(),
            text(value).font_size(17.).color(ColorToken::Muted),
        ]
        .height(52.)
        .padding(16., Edge::Horizontal)
        .align(Align::Center)
        .into()
    };
    shop_page(
        "checkout-page",
        vec![
            section(
                "Deliver to",
                vec![row("Home", "1 Market Street, Apt 4")],
                "",
            ),
            section(
                "Delivery time",
                vec![hstack![segmented_control([
                    (Window::Now, "Now"),
                    (Window::Tonight, "6–7 pm"),
                    (Window::Tomorrow, "Tomorrow"),
                ])
                .key("window")
                .selected(app.window)
                .on_select(|a: &mut App, w| a.window = w)
                .flex(1.)]
                .padding(16., Edge::Horizontal)
                .padding(10., Edge::Vertical)
                .into()],
                "",
            ),
            section(
                "Tip your shopper",
                vec![hstack![segmented_control([
                    (0u32, "None"),
                    (10, "10%"),
                    (15, "15%"),
                    (20, "20%"),
                ])
                .key("tip")
                .selected(app.tip)
                .on_select(|a: &mut App, t| a.tip = t)
                .flex(1.)]
                .padding(16., Edge::Horizontal)
                .padding(10., Edge::Vertical)
                .into()],
                "All of it goes to your shopper.",
            ),
            section(
                "Payment",
                vec![row("Apple Pay", "Visa •• 4242")],
                "A demo: no payment is made.",
            ),
            hstack![
                text("Total").font_size(20.).font_weight(700),
                spacer(),
                text(price(total(app)))
                    .key("total")
                    .font_size(20.)
                    .font_weight(700),
            ]
            .into(),
            button("Place order")
                .key("place-order")
                .variant(ButtonVariant::Primary)
                .font_size(17.)
                .font_weight(600)
                .height(52.)
                .rounded(26.)
                .on_click(|a: &mut App| {
                    a.ordered = cart_count(a);
                    a.cart.clear();
                    a.history.push(Route::Ordered);
                })
                .into(),
        ],
    )
}

pub fn ordered(app: &App) -> Container<App> {
    let when = match app.window {
        Window::Now => "in 15–25 minutes",
        Window::Tonight => "tonight between 6 and 7 pm",
        Window::Tomorrow => "tomorrow",
    };
    shop_page(
        "ordered-page",
        vec![vstack![
            zstack![icon(Icon::Check).size(44.).stroke(4.).color("#FFFFFF")]
                .width(96.)
                .height(96.)
                .background(BRAND)
                .rounded(48.),
            text("Order placed").font_size(28.).font_weight(700),
            text(format!(
                "{} items arriving {when}. We'll let you know when your shopper starts.",
                app.ordered
            ))
            .font_size(17.)
            .color(ColorToken::Muted),
            button("Back to the shop")
                .key("back-to-shop")
                .variant(ButtonVariant::Primary)
                .font_size(17.)
                .font_weight(600)
                .height(52.)
                .rounded(26.)
                .on_click(|a: &mut App| {
                    a.history.reset(Route::Home);
                    a.history.push(Route::Shop);
                }),
        ]
        .gap(14., Axis::Vertical)
        .align(Align::Center)
        .padding(48., Edge::Top)
        .into()],
    )
}
