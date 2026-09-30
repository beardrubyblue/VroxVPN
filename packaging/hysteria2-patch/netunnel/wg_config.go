package netunnel

import (
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"sort"
	"strings"
)

// defaultWireGuardMTU — если в конфиге нет MTU (wg-easy его не пишет).
// 1280, как официальный WireGuard для iOS (WireGuardKit/
// PacketTunnelSettingsGenerator: «too many broken networks out there»), а
// не десктопные 1420: на iPhone тоннель «подключался», но сети не было —
// крупные пакеты (TLS, страницы) молча терялись на путях с меньшим MTU
// (мобильный интернет, PPPoE), а мелкие (DNS) проходили; официальное
// приложение с тем же конфигом работало. Менять синхронно с
// wireguard/provider.rs::DEFAULT_MTU (MTU интерфейса в iOS).
const defaultWireGuardMTU = 1280

// obfuscationKeys — параметры маскировки AmneziaWG. В конфиге они пишутся
// с заглавной (Jc, S1, H1…), в UAPI — строчными; Rust уже приводит к
// строчным. Без них клиент работает как обычный WireGuard (wg-easy).
var obfuscationKeys = map[string]bool{
	"jc": true, "jmin": true, "jmax": true,
	"s1": true, "s2": true, "s3": true, "s4": true,
	"h1": true, "h2": true, "h3": true, "h4": true,
	"i1": true, "i2": true, "i3": true, "i4": true, "i5": true,
}

// WireGuardConfig — JSON от Rust (разобранный wg-quick .conf). Ключи —
// base64, как в .conf; Endpoint — уже IP:порт (имя резолвит Rust: внутри
// песочницы расширения DNS до поднятия тоннеля не работает, см. netunnel.go).
type WireGuardConfig struct {
	PrivateKey        string            `json:"privateKey"`
	MTU               int               `json:"mtu,omitempty"`
	Obfuscation       map[string]string `json:"obfuscation,omitempty"`
	Peer              WireGuardPeer     `json:"peer"`
	MemoryConstrained bool              `json:"memoryConstrained"`
	HistoryPath       string            `json:"historyPath,omitempty"`
}

type WireGuardPeer struct {
	PublicKey           string   `json:"publicKey"`
	PresharedKey        string   `json:"presharedKey,omitempty"`
	Endpoint            string   `json:"endpoint"`
	AllowedIPs          []string `json:"allowedIPs"`
	PersistentKeepalive int      `json:"persistentKeepalive,omitempty"`
}

// keyHex — base64-ключ из .conf в hex, как его ждёт UAPI.
func keyHex(name, base64Key string) (string, error) {
	raw, err := base64.StdEncoding.DecodeString(strings.TrimSpace(base64Key))
	if err != nil || len(raw) != 32 {
		return "", fmt.Errorf("netunnel: wireguard: invalid %s", name)
	}
	return hex.EncodeToString(raw), nil
}

// uapi — конфиг в формате WireGuard UAPI (device.IpcSet): параметры
// интерфейса (включая маскировку) до первой строки public_key, дальше пир.
func (c *WireGuardConfig) uapi() (string, error) {
	var b strings.Builder
	privateKey, err := keyHex("private key", c.PrivateKey)
	if err != nil {
		return "", err
	}
	fmt.Fprintf(&b, "private_key=%s\n", privateKey)

	keys := make([]string, 0, len(c.Obfuscation))
	for key := range c.Obfuscation {
		if !obfuscationKeys[key] {
			return "", fmt.Errorf("netunnel: wireguard: unknown parameter %q", key)
		}
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		fmt.Fprintf(&b, "%s=%s\n", key, c.Obfuscation[key])
	}

	publicKey, err := keyHex("peer public key", c.Peer.PublicKey)
	if err != nil {
		return "", err
	}
	fmt.Fprintf(&b, "replace_peers=true\npublic_key=%s\n", publicKey)
	if c.Peer.PresharedKey != "" {
		presharedKey, err := keyHex("preshared key", c.Peer.PresharedKey)
		if err != nil {
			return "", err
		}
		fmt.Fprintf(&b, "preshared_key=%s\n", presharedKey)
	}
	fmt.Fprintf(&b, "endpoint=%s\n", c.Peer.Endpoint)
	fmt.Fprintf(&b, "persistent_keepalive_interval=%d\n", c.Peer.PersistentKeepalive)
	b.WriteString("replace_allowed_ips=true\n")
	for _, allowedIP := range c.Peer.AllowedIPs {
		fmt.Fprintf(&b, "allowed_ip=%s\n", strings.TrimSpace(allowedIP))
	}
	return b.String(), nil
}
