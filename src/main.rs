use anyhow::{anyhow, bail, Context};
use chrono::{Datelike, Month, NaiveDate, Utc};
use clap::Parser;
use fs_err::File;
use itertools::Itertools;
use plotters::{backend::RGBPixel, coord::Shift, prelude::*};

use std::{
    collections::{BTreeMap, HashMap},
    io::{BufRead, BufReader},
    path::PathBuf,
};

const COLORS: [(u8, u8, u8); 7] = [
    (255, 0, 0),
    (0, 0, 255),
    (255, 255, 0),
    (144, 238, 144),
    (128, 0, 128),
    (255, 165, 0),
    (255, 20, 147),
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

type Stats = BTreeMap<u32, HashMap<String, Vec<f32>>>;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let data = File::open(args.data_file)
        .map(BufReader::new)
        .context("failed to open data file")?;

    let (year, month) = (args.year as i32, Month::number_from_month(&args.month));

    let (stats, ordered_categories) =
        calculate(data, (year, month)).context("failed to calculate")?;

    draw((year, month), stats, ordered_categories)?;

    Ok(())
}

fn calculate(
    data: impl BufRead,
    (year, month): (i32, u32),
) -> anyhow::Result<(Stats, Vec<String>)> {
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

    let ordered_categories = category_frequency
        .into_iter()
        .sorted_by_key(|(_category, freq)| *freq)
        .map(|(category, _freq)| category)
        .rev()
        .collect::<Vec<String>>();

    Ok((stats, ordered_categories))
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

fn draw(
    (year, month): (i32, u32),
    stats: Stats,
    ordered_categories: Vec<String>,
) -> anyhow::Result<()> {
    let colored_ordered_categories = ordered_categories
        .into_iter()
        .zip(COLORS.iter().copied().map(|(r, g, b)| RGBColor(r, g, b)))
        .collect::<Vec<(String, RGBColor)>>();

    let canvas = BitMapBackend::new("./pic.png", (640, 480)).into_drawing_area();
    canvas.fill(&WHITE)?;

    let canvas = canvas.margin(10, 10, 10, 10);

    draw_main_chart(stats.clone(), &colored_ordered_categories, &canvas)?;
    draw_avg_chart((year, month), stats, &colored_ordered_categories, &canvas)?;

    canvas.present()?;

    Ok(())
}

fn draw_avg_chart(
    (year, month): (i32, u32),
    stats: Stats,
    colored_ordered_categories: &[(String, RGBColor)],
    canvas: &DrawingArea<BitMapBackend<RGBPixel>, Shift>,
) -> anyhow::Result<()> {
    let x_range = 0u32..1u32;
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

    let mut chart = ChartBuilder::on(canvas)
        .margin_right(520)
        .caption("avg", ("sans-serif", 40).into_font())
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

    chart.draw_series([Rectangle::new([(0, 0.0), (1, 0.0)], BLACK)])?;

    let days = {
        let today = Utc::now();

        if (year, month) == (today.year(), today.month()) {
            today.day()
        } else {
            stats.len() as u32
        }
    };

    let mut totals = HashMap::<String, f32>::new();
    for (_day, day_stats) in stats {
        for (category, values) in day_stats {
            *totals.entry(category).or_default() += values.into_iter().sum::<f32>();
        }
    }

    let total = totals.values().sum::<f32>();
    let avg = total / days as f32;

    let mut series = vec![];
    let mut level = 0.0;
    for (category, color) in colored_ordered_categories {
        if let Some(ctotal) = totals.remove(category) {
            let adjusted_total = avg * (ctotal / total);
            series.push(Rectangle::new(
                [(0, level), (1, level + adjusted_total)],
                ShapeStyle {
                    color: (*color).into(),
                    filled: true,
                    stroke_width: 0,
                },
            ));
            level += adjusted_total;
        }
    }

    chart.draw_series(series)?;

    Ok(())
}

fn draw_main_chart(
    mut stats: Stats,
    colored_ordered_categories: &[(String, RGBColor)],
    canvas: &DrawingArea<BitMapBackend<RGBPixel>, Shift>,
) -> anyhow::Result<()> {
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

    let mut chart = ChartBuilder::on(canvas)
        .caption("main", ("sans-serif", 40).into_font())
        .margin_left(100)
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

    let mut totals = HashMap::<u32, f32>::new();
    for (category, color) in colored_ordered_categories {
        let style = ShapeStyle {
            color: (*color).into(),
            filled: true,
            stroke_width: 0,
        };

        let mut series = vec![];
        for (day, day_stats) in stats.iter_mut() {
            if let Some(values) = day_stats.remove(category) {
                let value = values.into_iter().sum::<f32>();
                let total = totals.get(day).copied().unwrap_or_default();

                series.push(Rectangle::new(
                    [(day - 1, total), (*day, total + value)],
                    style,
                ));

                totals
                    .entry(*day)
                    .and_modify(|t| {
                        *t += value;
                    })
                    .or_insert(value);
            }
        }

        if !series.is_empty() {
            chart
                .draw_series(series)?
                .legend(move |(x, y)| Circle::new((x, y), 3, style))
                .label(category);
        }
    }

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .margin(20)
        .legend_area_size(5)
        .border_style(BLUE)
        .background_style(BLUE.mix(0.1))
        .label_font(("Calibri", 20))
        .draw()
        .unwrap();

    Ok(())
}
