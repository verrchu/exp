# exp - and expense tracker I built for myself

be advised: the code here is bad and I am ok with that
I wanted to have a working thing in minimum time

here is how I use it. I enter my expenses into a regular text file in the following format
```
<day>
<category1> <amount>
<category2> <amount> <amount>

<day>
<category> <amount>
```

a single text file represents a single month

then I run this program on such file to get a graph of my expenses.

here is its interface:
```
Usage: exp_cli [OPTIONS] --month <MONTH> --year <YEAR> --output <OUTPUT> <DATA_FILE>

Arguments:
  <DATA_FILE>

Options:
  -m, --month <MONTH>
  -y, --year <YEAR>
  -o, --output <OUTPUT>
  -c, --chart <CHART>    [default: regular] [possible values: average-by-day, regular]
  -h, --help             Print help
```

so you must tell it for which exact month you want the graph so that it knows for example the number of days in the target months (useful for calculating average etc.)

there are two kinds of graphs:
* reguler - it shows xpenses by categories per day and also average expenses up until today if it is the ongoing month or average expenses per category for the whole month
* average by day - I also like to call it "floating average" though it is probably not what is usulally meant by this term. it present how average changed by category during the month

# exmplae
## regular graph
```
cargo run --bin exp_cli -- -m Jul -y 2023 -o reg.png jul-2023
```
![reg](https://github.com/verrchu/exp/assets/24650632/83c384a0-720b-4b5d-893d-6d7a4d6ba44d)


## average by day graph
```
cargo run --bin exp_cli -- -m Jul -y 2023 -c average-by-day -o avg.png jul-2023
```
![avg](https://github.com/verrchu/exp/assets/24650632/6509e925-4229-46ec-883b-0383bf6a6890)
