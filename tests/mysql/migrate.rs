use sqlx::migrate::Migrator;
use sqlx::mysql::{MySql, MySqlConnection};
use sqlx::pool::PoolConnection;
use sqlx::Executor;
use sqlx::Row;
use sqlx_test::new;
use std::fs;
use std::path::Path;

#[sqlx::test(migrations = false)]
async fn simple(mut conn: PoolConnection<MySql>) -> anyhow::Result<()> {
    clean_up(&mut conn).await?;

    let migrator = Migrator::new(Path::new("tests/mysql/migrations_simple")).await?;

    // run migration
    migrator.run(&mut conn).await?;

    // check outcome
    let res: String = conn
        .fetch_one("SELECT some_payload FROM migrations_simple_test")
        .await?
        .get(0);
    assert_eq!(res, "110_suffix");

    // running it a 2nd time should still work
    migrator.run(&mut conn).await?;

    Ok(())
}

#[sqlx::test(migrations = false)]
async fn reversible(mut conn: PoolConnection<MySql>) -> anyhow::Result<()> {
    clean_up(&mut conn).await?;

    let migrator = Migrator::new(Path::new("tests/mysql/migrations_reversible")).await?;

    // run migration
    migrator.run(&mut conn).await?;

    // check outcome
    let res: i64 = conn
        .fetch_one("SELECT some_payload FROM migrations_reversible_test")
        .await?
        .get(0);
    assert_eq!(res, 101);

    // roll back nothing (last version)
    migrator.undo(&mut conn, 20220721125033).await?;

    // check outcome
    let res: i64 = conn
        .fetch_one("SELECT some_payload FROM migrations_reversible_test")
        .await?
        .get(0);
    assert_eq!(res, 101);

    // roll back one version
    migrator.undo(&mut conn, 20220721124650).await?;

    // check outcome
    let res: i64 = conn
        .fetch_one("SELECT some_payload FROM migrations_reversible_test")
        .await?
        .get(0);
    assert_eq!(res, 100);

    Ok(())
}

#[sqlx::test(migrations = false)]
async fn skip() -> anyhow::Result<()> {
    let mut conn = new::<MySql>().await?;
    clean_up(&mut conn).await?;
    let migrator = Migrator::new(Path::new("tests/mysql/migrations_reversible")).await?;

    let fut = async move {
        let sql =
            fs::read_to_string("tests/mysql/migrations_reversible/20220721124650_add_table.up.sql")
                .unwrap();
        let statements: Vec<String> = sql
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        // Iterate by value, giving ownership of the String to the query in each loop.
        for statement in statements {
            // By passing `&statement`, you are creating a temporary borrow.
            // The future returned by `execute` will borrow `statement`.
            // BUT, because `statement` is owned by this loop iteration, it lives
            // long enough for the `.await` to complete. The borrow ends before the
            // next loop iteration, which is perfectly safe.
            conn.execute(statement.as_str()).await;
        }
    };

    // skip first migration
    migrator.skip(&mut conn, Some(20220721124650)).await?;

    // check outcome
    let res: i64 = conn
        .fetch_one("SELECT some_payload FROM migrations_reversible_test")
        .await?
        .get(0);
    assert_eq!(res, 100);

    // run remaining migration
    migrator.run(&mut conn).await?;

    // check outcome
    let res: i64 = conn
        .fetch_one("SELECT some_payload FROM migrations_reversible_test")
        .await?
        .get(0);
    assert_eq!(res, 101);

    // roll back one version
    migrator.undo(&mut conn, 20220721124650).await?;

    // check outcome
    let res: i64 = conn
        .fetch_one("SELECT some_payload FROM migrations_reversible_test")
        .await?
        .get(0);
    assert_eq!(res, 100);

    Ok(())
}

/// Ensure that we have a clean initial state.
async fn clean_up(conn: &mut MySqlConnection) -> anyhow::Result<()> {
    conn.execute("DROP TABLE migrations_simple_test").await.ok();
    conn.execute("DROP TABLE migrations_reversible_test")
        .await
        .ok();
    conn.execute("DROP TABLE _sqlx_migrations").await.ok();

    Ok(())
}
