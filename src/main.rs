use anyhow::{anyhow, bail, Context};
use chrono::{Month, NaiveDate};
use clap::Parser;
use fs_err::File;
use itertools::Itertools;

use std::{
    collections::{BTreeMap, HashMap},
    io::{BufRead, BufReader},
    iter,
    path::PathBuf,
};

const COLORS: [(u8, u8, u8); 7] = [
    (255, 0, 0),
    (0, 0, 255),
    (255, 255, 0),
    (0, 128, 0),
    (128, 0, 128),
    (255, 165, 0),
    (0, 128, 128),
];

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(short, long, value_parser = parse_month)]
    month: chrono::Month,
    #[clap(short, long)]
    year: u16,
    data_file: PathBuf,
}

fn parse_month(raw: &str) -> anyhow::Result<chrono::Month> {
    raw.parse().map_err(|_| anyhow!("failed to parse month"))
}

type Stats = BTreeMap<u32, Vec<(String, Vec<f32>)>>;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let data = File::open(args.data_file)
        .map(BufReader::new)
        .context("failed to open data file")?;

    let stats = calculate(
        data,
        (args.year as i32, Month::number_from_month(&args.month)),
    )
    .context("failed to calculate")?;

    draw(stats)?;

    Ok(())
}

fn calculate(data: impl BufRead, (year, month): (i32, u32)) -> anyhow::Result<Stats> {
    let mut stats = BTreeMap::<u32, HashMap<String, Vec<f32>>>::new();

    // TODO: move this code somewhere
    for day in 1..=31 {
        if NaiveDate::from_ymd_opt(year, month, day).is_some() {
            assert!(stats.insert(day, HashMap::new()).is_none());
        }
    }

    let mut category_frequency = HashMap::<String, usize>::new();

    let mut processing = false;
    let mut day = 0;

    for line in data.lines() {
        let line = line.context("failed to read line")?;
        let line = line.trim();

        if line.is_empty() {
            processing = false;
            continue;
        }

        if !processing {
            day = line
                .parse::<u32>()
                .context(format!("failed to parse day: {line}"))?;
            if let Some(day_stats) = stats.insert(day, HashMap::new()) {
                if !day_stats.is_empty() {
                    bail!("duplicate entries (day: {day})");
                }
            }

            processing = true;
            continue;
        }

        // unwrap: we have inserted this entry before
        let day_stats = stats.get_mut(&day).unwrap();

        let (category, values) = parse_data_line(line).context("failed to parse data line")?;
        category_frequency
            .entry(category.clone())
            .and_modify(|n| {
                *n += 1;
            })
            .or_insert(1);
        // TODO: remove this clone (possibly use `RefCell`
        if day_stats.insert(category.clone(), values).is_some() {
            bail!("duplicate category (day: {day}, category: {category})");
        }
    }

    let stats = stats
        .into_iter()
        .map(|(day, day_stats)| {
            let day_stats = day_stats
                .into_iter()
                .sorted_by_key(|(category, _values)| {
                    let freq = category_frequency.get(category).unwrap();
                    -(*freq as isize)
                })
                .collect();

            (day, day_stats)
        })
        .collect();

    Ok(stats)
}

fn parse_data_line(line: &str) -> anyhow::Result<(String, Vec<f32>)> {
    let mut tokens = line.split(' ');

    let category = tokens
        .next()
        .context("failed to extract category")?
        .to_string();
    let values = tokens
        .map(|val| val.parse::<f32>().context("failed to parse value"))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok((category, values))
}

fn draw(stats: Stats) -> anyhow::Result<()> {
    use plotters::prelude::*;

    let category_colors = stats
        .values()
        .map(|day_stats| day_stats.iter().map(|(category, _values)| category))
        .flatten()
        .unique()
        .cloned()
        .zip(COLORS.iter().copied().map(|(r, g, b)| RGBColor(r, g, b)))
        .collect::<HashMap<String, RGBColor>>();

    let root = BitMapBackend::new("./pic.png", (640, 480)).into_drawing_area();

    root.fill(&WHITE)?;

    let root = root.margin(10, 10, 10, 10);
    let x_range = 0u32..(stats.len() as u32);
    let y_range = {
        let max = stats
            .iter()
            .map(|(_, day_stats)| {
                day_stats
                    .iter()
                    .map(|(_category, values)| values.iter())
                    .flatten()
                    .sum::<f32>()
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or_default();

        0f32..max
    };
    let mut chart = ChartBuilder::on(&root)
        .caption("This is our first plot", ("sans-serif", 40).into_font())
        .x_label_area_size(20)
        .y_label_area_size(40)
        .build_cartesian_2d(x_range, y_range)?;

    chart
        .configure_mesh()
        // .disable_x_mesh()
        .bold_line_style(&WHITE.mix(0.3))
        .disable_x_axis()
        .set_tick_mark_size(LabelAreaPosition::Bottom, 0)
        // .y_desc("Count")
        // .x_desc("Bucket")
        // .axis_desc_style(("sans-serif", 15))
        .draw()?;

    chart.draw_series([Rectangle::new([(0, 0.0), (stats.len() as u32, 0.0)], BLACK)])?;

    let series = iter::once(Rectangle::new([(0, 0.0), (stats.len() as u32, 0.0)], BLACK))
        .chain(
            stats
                .iter()
                .map(|(day, day_stats)| {
                    let mut blocks = vec![];

                    let block_iter = day_stats
                        .iter()
                        .map(|(category, values)| {
                            values.iter().map(|value| (category.clone(), value))
                        })
                        .flatten();

                    let mut total = 0.0;
                    for (category, value) in block_iter {
                        blocks.push(Rectangle::new(
                            [(day - 1, total), (*day, total + value)],
                            ShapeStyle {
                                color: category_colors.get(&category).copied().unwrap().into(),
                                filled: true,
                                stroke_width: 0,
                            },
                        ));

                        total += value;
                    }

                    blocks
                })
                .flatten(),
        )
        .collect::<Vec<Rectangle<_>>>();

    chart.draw_series(series)?.label("this");

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperMiddle)
        .margin(20)
        .legend_area_size(5)
        .border_style(BLUE)
        .background_style(BLUE.mix(0.1))
        .label_font(("Calibri", 20))
        .draw()
        .unwrap();

    root.present()?;

    Ok(())
}
