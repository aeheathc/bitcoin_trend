'use strict';

const earliest_data_ts = 1325347200; //earliest is actually 1325346600 but this number rounds it off to the next hour
const startup_ts = Math.round(new Date().getTime()/1000);
const hour_in_seconds = 60 * 60;
const day_in_seconds = hour_in_seconds * 24;

//const starting_begin = startup_ts - day_in_seconds; //Sets chart min to 24 hours ago
const starting_begin = earliest_data_ts;              //Sets chart min to beginning of data
const starting_end = startup_ts;

let price_chart = undefined;
let chart_config = {
    type: 'line',
    data: {
        datasets: [{
            label: 'Bitcoin Price (USD)',
            data: [{x: 10, y:20},{x:20, y:30}],
            fill: false,
            cubicInterpolationMode: 'monotone'
        }]
    },
    options: {
        responsive: true,
        title: {
            display: true,
            text: 'Average Bitcoin prices over time from Bitstamp'
        },
        tooltips: {
            mode: 'index',
            intersect: false,
        },
        hover: {
            mode: 'nearest',
            intersect: true
        },
        scales: {
            xAxes: [{
                type: 'time',
                distribution: 'series',
                offset: true,
                display: true,
                scaleLabel: {
                    display: true,
                    labelString: 'Time (UTC)'
                },
                ticks: {
                    source: 'data',
                    autoSkip: true,
                    autoSkipPadding: 75,
                    maxRotation: 0,
                    sampleSize: 100,
                    major: {
                        enabled: true,
                        fontStyle: 'bold'
                    }
                }
            }],
            yAxes: [{
                display: true,
                scaleLabel: {
                    display: true,
                    labelString: 'Price (USD)'
                },
                ticks: {
                    beginAtZero: true
                }
            }]
        }
    }
};
let prices = new Map();    // timestamp_seconds => price_cents
let responses = new Map(); // "begin,end" => [..timestamps]

// Call once after the page is loaded to set up the chart and slider
function chart_init()
{
    //define the chart
    price_chart = new Chart($("#price_chart"), chart_config);

    //define 2-handle slider
    $("#slider").slider({
        range: true,
        min: earliest_data_ts,
        max: startup_ts,
        step: hour_in_seconds,
        values: [ starting_begin, starting_end ],
        change: function( event, ui ) {
            chart_update(ui.values[0], ui.values[1]);
        }
    });
    chart_update(starting_begin, starting_end);
}

/* Call each time you want to change the range displayed.
It will use AJAX to get data from the specified range, and cache the results for future calls,
then update the displayed range and the chart.
Params begin and end are unix timestamps in UTC.
*/
function chart_update(begin, end)
{
    //update date display for slider
    const begin_formatted = moment.utc(begin, "X").format("YYYY-MM-DD hh:mm:ss a") + " UTC";
    const end_formatted   = moment.utc(end, "X").format("YYYY-MM-DD hh:mm:ss a") + " UTC";
    $("#begin").html(begin_formatted);
    $("#end").html(end_formatted);

    //update chart
    const responseKey = begin + "," + end;
    
    if(responses.has(responseKey))
    {
        const times = responses.get(responseKey);
        let points = [];
        times.forEach(function(time){
            points.push({x:time, y:prices.get(time)});
        });

        chart_update_view(points, begin, end);
    }else{
        const spinner = $('#spinner');
        spinner.css('display','block');

        const endpoint = "/api/prices/" + begin + "/" + end;
        console.log(endpoint);
        $.ajax(endpoint)
            .done(function(msg, textStatus, xhrObj){
                let newTimes = [];
                let points = [];
                msg.forEach(function(row){
                    const time = row[0];
                    const price_cents = row[1];
                    newTimes.push(time);
                    prices.set(time, price_cents);
                    points.push({x:time, y:price_cents});
                });
                responses.set(responseKey, newTimes);
                chart_update_view(points, begin, end);
                $("#end").attr("title","");
            })
            .fail(function(xhrObj, statusStr, errorThrown){
                $("#end").attr("title",xhrObj.responseText);
            })
            .always(function(){
                spinner.css('display','none');
            });
    }
}

// Update the actual chart with the given data. Called by chart_update after it decides what data to use.
function chart_update_view(points, begin, end)
{
    if(points.length == 1)
    {
        points[0].x = begin;
        points.push({x:end, y:points[0].y});
    }
    
    //fudge the data for the current display only
    for(let i=0; i<points.length; ++i)
    {
        //Clamp to the requested range
        if(points[i].x < begin) points[i].x = begin;
        if(points[i].x > end) points[i].x = end;

        //convert timestamps to moment objects so the chart can handle them better
        //This also converts the timezone from UTC to local
        points[i].x = moment.unix(points[i].x).utc();

        //convert price to dollars
        points[i].y = (points[i].y / 100.0).toFixed(2);
    }

    chart_config.data.datasets.splice(0, 1);
    const newDataset = {
        label: 'Bitcoin Price (USD)',
        data: points,
        fill: false,
        cubicInterpolationMode: 'monotone'
    };
    chart_config.data.datasets.push(newDataset);
    price_chart.update();
}