package netunnel

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func readDays(t *testing.T, path string) []dayTraffic {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var days []dayTraffic
	if err := json.Unmarshal(data, &days); err != nil {
		t.Fatal(err)
	}
	return days
}

func TestAddDayTrafficAccumulatesSameDay(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.json")
	if err := addDayTraffic(path, "2026-09-30", 100, 1000); err != nil {
		t.Fatal(err)
	}
	if err := addDayTraffic(path, "2026-09-30", 5, 50); err != nil {
		t.Fatal(err)
	}
	days := readDays(t, path)
	if len(days) != 1 || days[0].Upload != 105 || days[0].Download != 1050 {
		t.Fatalf("got %+v", days)
	}
}

func TestAddDayTrafficKeepsLastMonthSorted(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.json")
	for day := 35; day >= 1; day-- { // в обратном порядке — проверяем сортировку
		date := fmt.Sprintf("2026-%02d-%02d", 8+day/32, (day-1)%31+1)
		if err := addDayTraffic(path, date, 1, 1); err != nil {
			t.Fatal(err)
		}
	}
	days := readDays(t, path)
	if len(days) != historyDays {
		t.Fatalf("want %d days, got %d", historyDays, len(days))
	}
	for i := 1; i < len(days); i++ {
		if days[i-1].Date >= days[i].Date {
			t.Fatalf("not sorted: %s >= %s", days[i-1].Date, days[i].Date)
		}
	}
}

func TestRecorderFlushWritesOnlyDelta(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.json")
	recorder := &historyRecorder{path: path}
	recorder.flush(100, 200)
	recorder.flush(100, 200) // ничего нового — не пишем повторно
	recorder.flush(150, 260)
	days := readDays(t, path)
	if len(days) != 1 || days[0].Upload != 150 || days[0].Download != 260 {
		t.Fatalf("got %+v", days)
	}
}
