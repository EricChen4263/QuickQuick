import hashlib
import hmac

# 独立实现：火山引擎 SigV4 风格签名（官方文档 volcengine.com/docs/6369/67269 「签名方法 V4」）
# 与 AWS 区别：算法名 "HMAC-SHA256"；credentialScope 以 "request" 结尾（非 aws4_request）；
# 签名密钥首层 = HMAC(secret_access_key, date)（不加 "AWS4" 前缀）；X-Date 格式 YYYYMMDDTHHMMSSZ。

def hmac_sha256(key, msg):
    return hmac.new(key, msg.encode('utf-8'), hashlib.sha256).digest()

def sha256_hex(msg):
    return hashlib.sha256(msg.encode('utf-8')).hexdigest()

def sign(access_key_id, secret_access_key, region, service, payload, timestamp):
    # timestamp: unix secs -> X-Date YYYYMMDDTHHMMSSZ (UTC)
    import time
    t = time.gmtime(timestamp)
    x_date = time.strftime('%Y%m%dT%H%M%SZ', t)
    short_date = time.strftime('%Y%m%d', t)

    host = "open.volcengineapi.com"
    method = "POST"
    canonical_uri = "/"
    # query string 按 key 排序，已 percent-encode
    canonical_query = "Action=TranslateText&Version=2020-06-01"
    content_type = "application/json"
    payload_hash = sha256_hex(payload)

    # CanonicalHeaders：content-type;host;x-content-sha256;x-date（按字母序）
    canonical_headers = (
        f"content-type:{content_type}\n"
        f"host:{host}\n"
        f"x-content-sha256:{payload_hash}\n"
        f"x-date:{x_date}\n"
    )
    signed_headers = "content-type;host;x-content-sha256;x-date"

    canonical_request = (
        f"{method}\n{canonical_uri}\n{canonical_query}\n"
        f"{canonical_headers}\n{signed_headers}\n{payload_hash}"
    )

    credential_scope = f"{short_date}/{region}/{service}/request"
    string_to_sign = (
        f"HMAC-SHA256\n{x_date}\n{credential_scope}\n{sha256_hex(canonical_request)}"
    )

    # 四层密钥派生
    k_date = hmac_sha256(secret_access_key.encode('utf-8'), short_date)
    k_region = hmac_sha256(k_date, region)
    k_service = hmac_sha256(k_region, service)
    k_signing = hmac_sha256(k_service, "request")

    signature = hmac.new(k_signing, string_to_sign.encode('utf-8'), hashlib.sha256).hexdigest()

    authorization = (
        f"HMAC-SHA256 Credential={access_key_id}/{credential_scope}, "
        f"SignedHeaders={signed_headers}, Signature={signature}"
    )
    return x_date, payload_hash, signature, authorization

payload = '{"SourceLanguage":"en","TargetLanguage":"zh","TextList":["hello"]}'
x_date, payload_hash, sig, auth = sign(
    "AKLTtest_access_key_id_123",
    "test_secret_access_key_abc",
    "cn-north-1",
    "translate",
    payload,
    1_700_000_000,
)
print("X-Date:", x_date)
print("payload_hash:", payload_hash)
print("signature:", sig)
print("authorization:", auth)
