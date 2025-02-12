const HEIGHT: f64 = 50.0;

pub fn render_graph(items: &[usize]) -> String {
    let mut html = String::new();
    html.push_str(
        r#"<svg style="font-size:12px; margin:0; padding:0;" width="240"  height="50"><rect style="fill: white; stroke-width:1; stroke:black;" width="240" height="50"></rect>"#,
    );

    if items.len() > 0 {
        let mut max = items[0];
        let mut min = items[0];

        for itm in items {
            let itm = *itm;
            if itm > max {
                max = itm
            }

            if itm < min {
                min = itm
            }
        }

        let mut x = 0;
        for itm in items {
            let itm = *itm as f64;

            let y1 = 50.0 - itm / HEIGHT * itm;

            html.push_str(format!(r#"<line x1="{}" y1="50" x2="{}" y2="{y1}" style="stroke:darkgray;stroke-width:2"></line>"#, x, x+1).as_str());
            x += 2;
        }

        html.push_str(format!(r#"<text fill="white" x="1" y="13">{max}</text>"#).as_str());
        html.push_str(format!(r#"<text fill="green" x="0" y="12">{max}</text>"#).as_str());
    }

    html.push_str("</svg>");

    html
}
