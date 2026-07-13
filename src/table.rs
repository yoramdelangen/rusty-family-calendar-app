pub(crate) fn print(headers: &[&str], rows: &[Vec<String>]) {
    let widths = column_widths(headers, rows);

    print_border(&widths);
    print_row(headers.iter().map(|value| value.to_string()).collect::<Vec<_>>().as_slice(), &widths);
    print_border(&widths);

    for row in rows {
        print_row(row, &widths);
    }

    print_border(&widths);
}

fn column_widths(headers: &[&str], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths = headers.iter().map(|header| header.len()).collect::<Vec<_>>();

    for row in rows {
        for (index, value) in row.iter().enumerate() {
            if index >= widths.len() {
                widths.push(value.len());
                continue;
            }

            widths[index] = widths[index].max(value.len());
        }
    }

    widths
}

fn print_border(widths: &[usize]) {
    print!("+");
    for width in widths {
        print!("-{}-+", "-".repeat(*width));
    }
    println!();
}

fn print_row(values: &[String], widths: &[usize]) {
    print!("|");
    for (index, width) in widths.iter().enumerate() {
        let value = values.get(index).map_or("", String::as_str);
        print!(" {:width$} |", value, width = *width);
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::column_widths;

    #[test]
    fn measures_column_widths() {
        let widths = column_widths(
            &["#", "Name"],
            &[
                vec!["10".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
            ],
        );

        assert_eq!(widths, vec![2, 5]);
    }
}
