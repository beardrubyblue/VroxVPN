interface RelayLimitsCardProps {
  idleTimeoutSeconds: number;
  onIdleTimeoutSecondsChange: (value: number) => void;
  maxTcpConnections: number;
  onMaxTcpConnectionsChange: (value: number) => void;
  maxUdpConnections: number;
  onMaxUdpConnectionsChange: (value: number) => void;
  connected: boolean;
}

// Лимиты relay-слоя на iOS (packaging/hysteria2-patch/netunnel/
// handler.go::applyRelayLimits) — дефолты синхронизированы с Happ
// (300с/256/128). Меняются только при отключённом VPN — Go-сторона
// читает их один раз при старте тоннеля, на лету не подхватит.
export function RelayLimitsCard({
  idleTimeoutSeconds,
  onIdleTimeoutSecondsChange,
  maxTcpConnections,
  onMaxTcpConnectionsChange,
  maxUdpConnections,
  onMaxUdpConnectionsChange,
  connected,
}: RelayLimitsCardProps) {
  return (
    <div>
      <div className="group-title">Производительность (iOS)</div>
      <div className="card">
        <div className="list-row">
          <span className="row-title">
            Idle timeout соединений
            <br />
            <span className="row-subtitle">Секунд без активности до закрытия, по умолчанию 300</span>
          </span>
          <input
            type="number"
            aria-label="Idle timeout соединений в секундах"
            className="number-input"
            value={idleTimeoutSeconds}
            disabled={connected}
            onChange={(e) => onIdleTimeoutSecondsChange(Number(e.target.value))}
          />
        </div>
        <div className="list-row">
          <span className="row-title">
            Максимум TCP-соединений
            <br />
            <span className="row-subtitle">По умолчанию 256</span>
          </span>
          <input
            type="number"
            aria-label="Максимум TCP-соединений"
            className="number-input"
            value={maxTcpConnections}
            disabled={connected}
            onChange={(e) => onMaxTcpConnectionsChange(Number(e.target.value))}
          />
        </div>
        <div className="list-row">
          <span className="row-title">
            Максимум UDP-соединений
            <br />
            <span className="row-subtitle">По умолчанию 128</span>
          </span>
          <input
            type="number"
            aria-label="Максимум UDP-соединений"
            className="number-input"
            value={maxUdpConnections}
            disabled={connected}
            onChange={(e) => onMaxUdpConnectionsChange(Number(e.target.value))}
          />
        </div>
      </div>
    </div>
  );
}
