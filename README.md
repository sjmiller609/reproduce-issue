## Reproduce CPU consumption issue


### Start the services using Docker Compose:

```bash
docker compose up --build
```

### Access the Flame Graph:
- Open your web browser
- Navigate to http://localhost:8080/debug/pprof/profile
- You should see the flame graph visualization
- Wait, check again later. CPU utilization should be growing over time unexpectedly.
