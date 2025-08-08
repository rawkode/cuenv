use cuenv::tui::EventBus;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_tui_no_lockup_between_tasks() {
    // Create event bus
    let event_bus = EventBus::new();
    EventBus::set_global(event_bus.clone()).ok();

    // Register two test tasks
    event_bus.register_task("task1".to_string(), vec![]).await;
    event_bus
        .register_task("task2".to_string(), vec!["task1".to_string()])
        .await;

    // Simulate task execution events
    tokio::spawn(async move {
        let bus = EventBus::global().unwrap();

        // Start task1
        bus.publish(cuenv::tui::events::TaskEvent::Started {
            task_name: "task1".to_string(),
            timestamp: std::time::Instant::now(),
        })
        .await;

        // Add some logs
        for i in 0..5 {
            bus.publish(cuenv::tui::events::TaskEvent::Log {
                task_name: "task1".to_string(),
                stream: cuenv::tui::events::LogStream::Stdout,
                content: format!("Task 1 log line {i}"),
            })
            .await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Complete task1
        bus.publish(cuenv::tui::events::TaskEvent::Completed {
            task_name: "task1".to_string(),
            exit_code: 0,
            duration_ms: 100,
        })
        .await;

        // Small delay before starting task2
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Start task2 - this is where the lockup was happening
        bus.publish(cuenv::tui::events::TaskEvent::Started {
            task_name: "task2".to_string(),
            timestamp: std::time::Instant::now(),
        })
        .await;

        // Add logs for task2
        for i in 0..5 {
            bus.publish(cuenv::tui::events::TaskEvent::Log {
                task_name: "task2".to_string(),
                stream: cuenv::tui::events::LogStream::Stdout,
                content: format!("Task 2 log line {i}"),
            })
            .await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Complete task2
        bus.publish(cuenv::tui::events::TaskEvent::Completed {
            task_name: "task2".to_string(),
            exit_code: 0,
            duration_ms: 100,
        })
        .await;
    });

    // Test that we can process events without lockup
    let mut subscriber = event_bus.subscribe();

    // This should not timeout if the fix works
    let result = timeout(Duration::from_secs(2), async {
        let mut events_received = 0;
        while let Some(_event) = subscriber.recv().await {
            events_received += 1;
            if events_received >= 14 {
                // We expect at least 14 events (2 starts + 10 logs + 2 completes)
                break;
            }
        }
        events_received
    })
    .await;

    assert!(
        result.is_ok(),
        "TUI event processing timed out - possible lockup!"
    );
    assert!(result.unwrap() >= 14, "Not all events were processed");
}
