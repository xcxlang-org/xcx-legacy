# 15. Practical Examples

Complete, working examples showing how XCX features combine in real applications.

## 15.1. Library Management System

```xcx
# Data model:
let (table) books {
    columns = [
        id :: int @auto,
        title :: str,
        author :: str,
        isbn :: str,
        publication_year :: int,
        available :: boolean
    ]
    rows = [ EMPTY ]
};

# System functions:
dec func (add_book :: let (str) title, let (str) author, let (str) isbn, let (int) year) {
    # ISBN validation (simple version)
    if (isbn.length() != 13) then;
        >! "ERROR: ISBN must be 13 characters";
        halt;
    end;

    books.insert(title, author, isbn, year, true);
    >! "Book added: " + title;
};

dec func (borrow_book :: let (int) book_id) {
    let (table) to_borrow = books.where(id == book_id AND available == true);

    if (to_borrow.count() == 0) then;
        >! "Book unavailable or does not exist";
        return;
    end;

    books.update(book_id, [to_borrow[0].title, to_borrow[0].author,
                           to_borrow[0].isbn, to_borrow[0].publication_year, false]);
    >! "Borrowed: " + to_borrow[0].title;
};

# Usage:
add_book("War and Peace", "Leo Tolstoy", "9780140447934", 1869);
add_book("1984", "George Orwell", "9780451524935", 1949);

borrow_book(1);

let (table) available_books = books.where(available == true);
>! "Available books: " + available_books.count();
```

## 15.2. Sales Data Analysis

```xcx
let (table) sales {
    columns = [
        id :: int @auto,
        product :: str,
        quantity :: int,
        unit_price :: float,
        sale_date :: date
    ]
    rows = [ EMPTY ]
};

sales.insert("Laptop", 2, 2999.99, date("2024-01-15"));
sales.insert("Phone", 5, 1499.50, date("2024-01-16"));
sales.insert("Laptop", 1, 2999.99, date("2024-01-17"));

dec func (sales_analysis) {
    # Total sales value:
    let (float) total = 0;
    let (int) i = 0;

    while (i < sales.count()) do;
        let (float) transaction_value = sales[i].quantity * sales[i].unit_price;
        total = total + transaction_value;
        i = i + 1;
    end;

    >! "Total sales: " + total;

    # Units sold per product:
    let (map :: str <-> int) unit_count = {};

    i = 0;
    while (i < sales.count()) do;
        let (str) prod = sales[i].product;
        if (unit_count.has(prod)) then;
            let (int) current = unit_count.get(prod);
            unit_count.insert(prod, current + sales[i].quantity);
        els;
            unit_count.insert(prod, sales[i].quantity);
        end;
        i = i + 1;
    end;

    >! "Sales statistics by product:";
    for (str) product in unit_count.keys();
        >! "  " + product + ": " + unit_count.get(product) + " units";
    end;
};

sales_analysis;
```

## 15.3. Project Planning Application

```xcx
let (table) tasks {
    columns = [
        id :: int @auto,
        name :: str,
        description :: str,
        priority :: int,
        status :: str,  # "todo", "in_progress", "completed"
        deadline :: date,
        assigned_to :: str
    ]
    rows = [ EMPTY ]
};

dec func (add_task :: let (str) name, let (str) description, let (int) priority, let (date) deadline) {
    tasks.insert(name, description, priority, "todo", deadline, "unassigned");
};

dec func (assign_task :: let (int) task_id, let (str) employee) {
    let (table) to_assign = tasks.where(id == task_id);

    if (to_assign.count() == 0) then;
        >! "Task does not exist";
        return;
    end;

    tasks.update(task_id, [to_assign[0].name, to_assign[0].description,
                           to_assign[0].priority, to_assign[0].status,
                           to_assign[0].deadline, employee]);
};

dec func (complete_task :: let (int) task_id) {
    tasks.update(task_id, [tasks[task_id].name, tasks[task_id].description,
                           tasks[task_id].priority, "completed",
                           tasks[task_id].deadline, tasks[task_id].assigned_to]);
};

dec func (urgent_tasks) {
    let (date) tomorrow = date.now() + 1;
    return tasks.where(deadline <= tomorrow AND status != "completed");
};

# Usage:
add_task("Prepare report", "Monthly sales report", 1, date.now() + 3);
add_task("Client meeting", "Product presentation", 2, date.now() + 1);

assign_task(1, "John Smith");
complete_task(2);

let (table) urgent = urgent_tasks();
>! "Urgent tasks: " + urgent.count();
```

## 15.4. Temperature Monitoring System

```xcx
let (table) temperature_readings {
    columns = [
        id :: int @auto,
        sensor_id :: str,
        temperature :: float,
        humidity :: float,
        pressure :: float,
        reading_date :: date,
        time :: str
    ]
    rows = [ EMPTY ]
};

# Alarm thresholds:
let (map :: str <-> float) alarm_thresholds = {
    "temperature_min" :: 18.0,
    "temperature_max" :: 25.0,
    "humidity_max" :: 60.0
};

dec func (add_reading :: let (str) sensor, let (float) temp, let (float) hum, let (float) press) {
    let (date) today = date.now();
    let (str) time = "14:30";  # In practice fetched from system

    temperature_readings.insert(sensor, temp, hum, press, today, time);

    # Check alarms:
    let (float) min_threshold = alarm_thresholds.get("temperature_min");
    let (float) max_threshold = alarm_thresholds.get("temperature_max");
    let (float) hum_max = alarm_thresholds.get("humidity_max");

    if (temp < min_threshold OR temp > max_threshold) then;
        >! "WARNING: Temperature " + temp + " out of range in sensor " + sensor;
    end;

    if (hum > hum_max) then;
        >! "WARNING: Humidity " + hum + "% exceeds limit in sensor " + sensor;
    end;
};

dec func (daily_report :: let (date) date_param) {
    let (table) daily = temperature_readings.where(reading_date == date_param);

    >! "=== DAILY REPORT " + date_param.format("DD-MM-YYYY") + " ===";
    >! "Number of readings: " + daily.count();

    if (daily.count() > 0) then;
        let (float) temp_sum = 0;
        let (int) i = 0;

        while (i < daily.count()) do;
            temp_sum = temp_sum + daily[i].temperature;
            i = i + 1;
        end;

        let (float) average = temp_sum / daily.count();
        >! "Average temperature: " + average + "°C";
    end;
};

# Simulation:
add_reading("S-001", 22.5, 45.0, 1013.25);
add_reading("S-002", 26.8, 50.0, 1012.80);  # Alarm: too hot
add_reading("S-003", 21.2, 65.0, 1014.10);  # Alarm: humidity

daily_report(date.now());
```
