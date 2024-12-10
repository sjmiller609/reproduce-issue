## Reproduce query rate issues

### Start the services using Docker Compose:

```bash
docker-compose up --build
```

### Check query rate:

Run the below command a few times, checking the change in the "calls" column of the output

```bash
 docker exec -u postgres -it investigate-jobs-db-1 psql -c 'SELECT
    query,
    calls,
    total_exec_time,
    total_exec_time/calls as avg_exec_time,
    rows/calls as avg_rows,
    100.0 * shared_blks_hit/nullif(shared_blks_hit + shared_blks_read, 0) AS hit_percent
FROM pg_stat_statements where calls > 100
ORDER BY total_exec_time DESC
LIMIT 10;'
```

### Access the Flame Graph:

http://localhost:8080/debug/pprof/profile
