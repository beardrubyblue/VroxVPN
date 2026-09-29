package netunnel

import (
	"encoding/json"
	"os"
	"sort"
	"sync"
	"sync/atomic"
	"time"
)

// История трафика по дням — пишет само расширение, а не приложение: оно
// видит каждый байт тоннеля, даже когда приложение закрыто/свёрнуто и
// ничего не опрашивает. Файл лежит в App Group (путь передаёт Swift в
// Config.HistoryPath), приложение читает его оттуда же
// (app/src-tauri/src/traffic_history.rs). Формат общий с Linux-записью
// в Rust — менять синхронно.
//
// Точность: дельта счётчиков сбрасывается в файл раз в
// historyFlushInterval, при sleep() устройства и при Stop() — потерять
// можно только хвост ≤ интервала, если iOS убьёт процесс без Stop.

const (
	// historyDays — ровно месяц, по столбику на день на экране Stats;
	// старше в файле не храним.
	historyDays          = 30
	historyFlushInterval = 5 * time.Second
	historyDateLayout    = "2006-01-02"
)

type dayTraffic struct {
	Date     string `json:"date"`
	Upload   uint64 `json:"upload"`
	Download uint64 `json:"download"`
}

// historyRecorder — сколько байт уже записано в файл за эту сессию
// тоннеля; дельта между счётчиками и этими значениями и есть «новый»
// трафик для текущего дня.
type historyRecorder struct {
	mu            sync.Mutex
	path          string
	savedUpload   uint64
	savedDownload uint64
}

// FlushHistory — точка входа для Swift (sleep()/stopTunnel): записать
// накопленную дельту немедленно. gomobile экспортирует как flushHistory().
func (h *TunnelHandle) FlushHistory() {
	if h.history == nil {
		return
	}
	h.history.flush(atomic.LoadUint64(&h.txBytes), atomic.LoadUint64(&h.rxBytes))
}

func (h *TunnelHandle) recordHistoryPeriodically() {
	ticker := time.NewTicker(historyFlushInterval)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			h.FlushHistory()
		case <-h.stopHistory:
			return
		}
	}
}

func (r *historyRecorder) flush(upload, download uint64) {
	r.mu.Lock()
	defer r.mu.Unlock()
	deltaUp, deltaDown := upload-r.savedUpload, download-r.savedDownload
	if deltaUp == 0 && deltaDown == 0 {
		return
	}
	if err := addDayTraffic(r.path, time.Now().Format(historyDateLayout), deltaUp, deltaDown); err != nil {
		return // попробуем на следующем тике — дельта не потеряется
	}
	r.savedUpload, r.savedDownload = upload, download
}

// addDayTraffic прибавляет трафик к дню и оставляет последние historyDays
// дней. Запись через временный файл + rename: приложение может читать
// файл в любой момент и не должно увидеть его наполовину записанным.
func addDayTraffic(path, date string, upload, download uint64) error {
	var days []dayTraffic
	if data, err := os.ReadFile(path); err == nil {
		_ = json.Unmarshal(data, &days) // битый файл — начинаем заново
	}

	found := false
	for i := range days {
		if days[i].Date == date {
			days[i].Upload += upload
			days[i].Download += download
			found = true
			break
		}
	}
	if !found {
		days = append(days, dayTraffic{Date: date, Upload: upload, Download: download})
	}
	sort.Slice(days, func(i, j int) bool { return days[i].Date < days[j].Date })
	if len(days) > historyDays {
		days = days[len(days)-historyDays:]
	}

	data, err := json.Marshal(days)
	if err != nil {
		return err
	}
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}
