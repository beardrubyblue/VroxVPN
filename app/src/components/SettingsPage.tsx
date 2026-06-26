import { Switch } from "@/components/Switch";
import type { UpdateInfo } from "@/types";

interface SettingsPageProps {
  ruBypass: boolean;
  onRuBypassChange: (checked: boolean) => void;
  connected: boolean;
  geoipLoading: boolean;
  onUpdateGeoip: () => void;
  geositeLoading: boolean;
  onUpdateGeosite: () => void;
  bypassStatus: string;
  killSwitch: boolean;
  onKillSwitchChange: (checked: boolean) => void;
  updateChecking: boolean;
  onCheckUpdate: () => void;
  updateInfo: UpdateInfo | null;
  updateInstalling: boolean;
  onInstallUpdate: () => void;
  onQuit: () => void;
}

export function SettingsPage({
  ruBypass,
  onRuBypassChange,
  connected,
  geoipLoading,
  onUpdateGeoip,
  geositeLoading,
  onUpdateGeosite,
  bypassStatus,
  killSwitch,
  onKillSwitchChange,
  updateChecking,
  onCheckUpdate,
  updateInfo,
  updateInstalling,
  onInstallUpdate,
  onQuit,
}: SettingsPageProps) {
  return (
    <main className="page">
      <div>
        <div className="group-title">Маршрутизация</div>
        <div className="card">
          <div className="list-row">
            <span className="row-title">
              Российские сервисы напрямую
              <br />
              <span className="row-subtitle">По IP (geoip) и по списку доменов (geosite)</span>
            </span>
            <Switch checked={ruBypass} onChange={onRuBypassChange} disabled={connected} />
          </div>
          <div className="list-row">
            <span className="row-title">База IP-адресов России</span>
            <div className="row-actions">
              <button className="plain-button" disabled={geoipLoading} onClick={onUpdateGeoip}>
                {geoipLoading ? <span className="spinner" /> : "Обновить"}
              </button>
            </div>
          </div>
          <div className="list-row">
            <span className="row-title">Список доменов российских сервисов</span>
            <div className="row-actions">
              <button className="plain-button" disabled={geositeLoading} onClick={onUpdateGeosite}>
                {geositeLoading ? <span className="spinner" /> : "Обновить"}
              </button>
            </div>
          </div>
          {bypassStatus && (
            <div className="list-row">
              <span className="muted-note">{bypassStatus}</span>
            </div>
          )}
        </div>
      </div>

      <div>
        <div className="group-title">Защита</div>
        <div className="card">
          <div className="list-row">
            <span className="row-title">
              Kill Switch
              <br />
              <span className="row-subtitle">Блокировать трафик без VPN</span>
            </span>
            <Switch checked={killSwitch} onChange={onKillSwitchChange} disabled={connected} />
          </div>
        </div>
      </div>

      <div>
        <div className="group-title">О программе</div>
        <div className="card">
          <div className="list-row">
            <span className="row-title">vrox.vpn 4.0.0</span>
          </div>
          <div className="list-row">
            <button className="plain-button" disabled={updateChecking} onClick={onCheckUpdate}>
              {updateChecking ? <span className="spinner" /> : "Проверить обновления"}
            </button>
          </div>
          {updateInfo && (
            <div className="list-row">
              <span className="row-title">
                Версия {updateInfo.version}
                {updateInfo.notes && (
                  <>
                    <br />
                    <span className="row-subtitle">{updateInfo.notes}</span>
                  </>
                )}
              </span>
              {updateInfo.autoInstallable ? (
                <button className="plain-button" disabled={updateInstalling} onClick={onInstallUpdate}>
                  {updateInstalling ? <span className="spinner" /> : "Установить"}
                </button>
              ) : (
                <span className="muted-note">через TestFlight</span>
              )}
            </div>
          )}
        </div>
      </div>

      <div>
        <div className="card">
          <div className="list-row">
            <button className="plain-button destructive-text" onClick={onQuit}>
              Закрыть полностью
            </button>
          </div>
        </div>
      </div>
    </main>
  );
}
