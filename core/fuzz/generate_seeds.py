#!/usr/bin/env python3
"""Write fuzz seeds to core/fuzz/seeds from Core test fixtures and test vectors."""

import base64
import binascii
import json
import pathlib
import shutil
import struct

CORE = pathlib.Path(__file__).resolve().parent.parent
SEEDS = CORE / "fuzz" / "seeds"

BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
TEST_PRIVATE_KEY = bytes([1] * 32)
TEST_PUBLIC_KEY = bytes.fromhex("8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c")

EVM = "0xcB3028d6120802148f03d6c884D6AD6A210Df62A"
SOL = "HA4hQMs22nCuRN7iLDBsBkboz2SnLM1WkNtzLo6xEDY5"
TON = "UQA5olhYULHkui4mTQM0LodWG0EqUaxmK6-e3mHrCZFO2diA"
TON_SIGNER = "UQDvr_S6wiD4iy6Y6x2c_8yjv-O2bs4xp9bFiQ0w39evpYZV"
TRON = "TJoSEwEqt7cT3TUwmEoUYnYs5cZR3xSukM"
SUI = "0xfa92fe9555eeb34d3d922dae643483cbd18bd607bf900a1df5e82dc22804698e"
SOL_CHAIN = "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"
SIWE = f"""app.example.com wants you to sign in with your Ethereum account:
{EVM}

Sign in to Example

URI: https://app.example.com
Version: 1
Chain ID: 1
Nonce: 32891756
Issued At: 2026-01-01T00:00:00Z"""

EIP712_FIXTURES = [
    "crates/gem_evm/testdata/eip712_canonical_chain_id_1.json",
    "crates/gem_evm/testdata/eip712_arrays_nested.json",
    "crates/gem_evm/testdata/eip712_signed_integers.json",
    "crates/gem_evm/testdata/eip712_reference_vector.json",
    "crates/gem_evm/testdata/uniswap_permit2.json",
    "crates/gem_evm/testdata/1inch_permit.json",
    "gemstone/src/message/test/eip712_seaport.json",
    "gemstone/src/message/test/eip712_polymarket.json",
    "crates/gem_hypercore/testdata/hl_eip712_approve_agent.json",
]
SOLANA_TRANSACTION = "crates/gem_wallet_connect/testdata/solana_transfer_transaction.json"
KEYSTORE_FIXTURES = ["v3_ios_mnemonic.json", "v3_ios_private_key.json", "v3_empty_salt_mnemonic.json", "v3_minimal_template.json"]


def fixture(path):
    return (CORE / path).read_bytes()


def text(path):
    return fixture(path).decode()


def write(target, name, data):
    directory = SEEDS / target
    directory.mkdir(parents=True, exist_ok=True)
    (directory / name).write_bytes(data.encode() if isinstance(data, str) else data)


def base58(data):
    number = int.from_bytes(data, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = BASE58_ALPHABET[remainder] + encoded
    return "1" * (len(data) - len(data.lstrip(b"\0"))) + encoded


def stellar_secret(key):
    payload = bytes([18 << 3]) + key
    return base64.b32encode(payload + struct.pack("<H", binascii.crc_hqx(payload, 0))).decode()


def payment_urls():
    return [
        "bitcoin:bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4?amount=0.001&label=Shop&lightning=lnbc420bogusinvoice",
        "bitcoin:?lno=lno1bogusoffer&sp=sp1qsilentpayment",
        "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
        "dogecoin:DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L?amount=42",
        "litecoin:ltc1qg82tyxq6y4hgk7wqfp9ts0v5x0x3x3t5l0wd8g?amount=1.5",
        f"ethereum:{EVM}?value=1e18",
        "ethereum:0x32Be343B94f860124dC4fEe278FDCBD38C102D88?value=10&gas=200000&gasPrice=20000000000",
        f"ethereum:0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48@1/transfer?address={EVM}&uint256=1.5e6",
        "ethereum:pay-gemwallet.eth@1",
        "polygon:0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326",
        f"eip155:1:{EVM}",
        "0x25851Bf7D35293A89F710eBFbD4718322eF7B174?amount=50.72",
        f"solana:{SOL}?amount=1&spl-token=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&label=Michael&memo=OrderId5678",
        f"solana:{SOL}?amount=0.266232&reference={SOL}",
        "solana:https%3A%2F%2Fmerchant.example%2Fpay%3Forder%3D12345",
        f"ton://transfer/{TON}?amount=1000000000&text=order+7",
        f"ton://transfer/{TON}?jetton=EQBlqsm144Dq6SjbPI4jjZvA1hqTIP3CvHovbIfW_t-SCALE&amount=5000000&text=hello",
        "ripple:rEb8TK3gBgk5auZkwc6sHnwrGVJH8DuaLh?amount=1&dt=12345",
        "xrpl:rEb8TK3gBgk5auZkwc6sHnwrGVJH8DuaLh",
        "web+stellar:pay?destination=GABC",
        "cosmos:cosmos1qypqxpq9qcrsszg2pvxq6rs0zqg3yyc5lzv7xu",
        "wc:abc@2?pay=https%3A%2F%2Fpay.walletconnect.com%2F%3Fpid%3Dpay_123",
        "wc:abc123@2?relay-protocol=irn&symKey=deadbeef",
        "https://pay.walletconnect.com/?pid=pay_123",
        "https://pay.walletconnect.com/pay_123",
        "gem://wc?sessionTopic=abc",
        "https://gemwallet.com/tokens/bitcoin",
    ]


def addresses():
    return [
        "0x5615e8ab93b9d695b6d4d6545f7792aa59e1069a",
        EVM,
        "cosmos1h3laqcrmul79zwtw6j63ncsl0adfj07wgupylj",
        "GvhwZwtV32kYUXUw965CUM3KGPdtBsDwPVpi92brY5R2",
        "rnBFvgZphmN39GWzUJeUitaP22Fr9be75H",
        "h3rman.near",
        "UQAzoUpalAaXnVm5MoiYWRZguLFzY0KxFjLv3MkRq5BXz3VV",
        "0:335996ba9cce6625ebfdd7019ced50927c1a08b1846a289a55efc37c01d77df9",
        "15e6w4u9nH4Tb9HdJco2Zua4y5DpHb1hHXBKBGkUrLMTpuXo",
        "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
        "bitcoincash:qpzl3jxkzgvfd9flnd26leud5duv795fnv7vuaha70",
        "addr1q8043m5heeaydnvtmmkyuhe6qv5havvhsf0d26q3jygsspxlyfpyk6yqkw0yhtyvtr0flekj84u64az82cufmqn65zdsylzk23",
        TRON,
        SUI,
        "DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L",
        f"eip155:8453:{EVM}",
        f"{SOL_CHAIN}:{SOL}",
    ]


def private_keys():
    return [
        "0x" + TEST_PRIVATE_KEY.hex(),
        TEST_PRIVATE_KEY.hex(),
        base58(TEST_PRIVATE_KEY + TEST_PUBLIC_KEY),
        "ed25519:" + base58(TEST_PRIVATE_KEY),
        stellar_secret(TEST_PRIVATE_KEY),
        "not_valid",
    ]


def keystore_v4_header():
    file = {
        "version": 4,
        "id": "4b8fc216-dc74-481e-8011-bd4d1bc968ab",
        "kind": "mnemonic",
        "crypto": {
            "kdf": {"algorithm": "argon2id", "memory_kib": 19456, "iterations": 2, "parallelism": 1, "salt": "00" * 16, "output_len": 32},
            "cipher": {"algorithm": "aes-256-gcm", "nonce": "00" * 12, "tag_len": 16},
            "ciphertext": "00" * 48,
        },
    }
    return json.dumps(file, indent=2)


def ton_send_message():
    return json.dumps({"valid_until": 9999999999, "messages": [{"address": "0:1234567890abcdef", "amount": "1000000000"}]})


def wallet_connect_requests():
    def typed_data(path):
        return json.dumps([EVM, text(path)])

    return [
        ("personal_sign", "eip155:1", json.dumps(["0x48656c6c6f", EVM])),
        ("personal_sign", "eip155:1", json.dumps(["0x" + SIWE.encode().hex(), EVM])),
        ("eth_signTypedData_v4", "eip155:1", typed_data(EIP712_FIXTURES[0])),
        ("eth_signTypedData_v4", "eip155:1", typed_data(EIP712_FIXTURES[4])),
        ("eth_signTypedData", "eip155:1", typed_data(EIP712_FIXTURES[6])),
        ("eth_sendTransaction", "eip155:1", json.dumps([json.loads(fixture("crates/gem_wallet_connect/testdata/ethereum_approval.json"))])),
        ("eth_signTransaction", "eip155:8453", json.dumps([{"from": EVM, "to": EVM, "value": "0x1", "gas": "0x5208", "nonce": "0x0", "chainId": "0x2105"}])),
        ("wallet_switchEthereumChain", "", json.dumps([{"chainId": "0x1"}])),
        ("wallet_addEthereumChain", "", json.dumps([{"chainId": "0x13e31", "chainName": "Blast"}])),
        ("eth_chainId", "eip155:1", "[]"),
        ("solana_signMessage", SOL_CHAIN, json.dumps({"message": base58(SIWE.encode()), "pubkey": SOL})),
        ("solana_signTransaction", SOL_CHAIN, text(SOLANA_TRANSACTION)),
        ("solana_signAllTransactions", SOL_CHAIN, text("crates/gem_wallet_connect/testdata/solana_sign_all_transactions.json")),
        ("sui_getAccounts", "sui:mainnet", "{}"),
        ("sui_signPersonalMessage", "sui:mainnet", json.dumps({"message": "Hello Sui"})),
        ("sui_signTransaction", "sui:mainnet", json.dumps({"address": SUI, "transaction": "AAACAAhkAAA"})),
        ("ton_signData", "ton:-239", text("crates/gem_wallet_connect/testdata/ton_connect_sign_data.json")),
        ("ton_signData", "ton:-239", json.dumps([{"type": "binary", "bytes": base64.b64encode(b"hello").decode(), "from": TON_SIGNER}])),
        ("ton_sendMessage", "ton:-239", ton_send_message()),
        ("tron_signMessage", "tron:0x2b6653dc", json.dumps({"message": "Hello"})),
        ("tron_sendTransaction", "tron:0x2b6653dc", json.dumps(text("crates/gem_wallet_connect/testdata/tron_send_transaction.json").strip())),
    ]


def wallet_connect_transactions():
    ton = {"valid_until": 9999999999, "messages": [{"address": TON, "amount": "1000000000", "payload": "te6cckEBAQEAAgAAAEysuc0="}]}
    return [
        ("ethereum", text("crates/gem_wallet_connect/testdata/ethereum_approval.json")),
        ("solana", text(SOLANA_TRANSACTION)),
        ("solana:signature", text(SOLANA_TRANSACTION)),
        ("sui", json.dumps({"address": SUI, "transaction": "AAACAAhkAAA"})),
        ("ton", json.dumps(ton)),
        ("tron", text("crates/gem_wallet_connect/testdata/tron_send_transaction.json")),
        ("tron:signature", json.dumps({"transaction": {"raw_data_hex": "abc"}})),
    ]


def sign_messages():
    def ton_sign_data(payload, address):
        return json.dumps({"payload": payload, "domain": "app.example.com", "address": address})

    messages = [
        ("eip191", "Hello"),
        ("eip191", "0x48656c6c6f"),
        ("siwe", SIWE),
        ("siwe", "0x" + SIWE.encode().hex()),
        ("base58", base58(fixture("crates/gem_solana/testdata/siws_complete.txt"))),
        ("base58", base58(fixture("crates/gem_solana/testdata/siws_sign_in.txt"))),
        ("sui", "Hello Sui"),
        ("ton", ton_sign_data({"type": "text", "text": "Hello TON"}, TON_SIGNER)),
        ("ton", ton_sign_data({"type": "binary", "bytes": base64.b64encode(b"hello").decode()}, TON)),
        ("ton", ton_sign_data({"type": "cell", "schema": "message#_ text:^Cell = Message;", "cell": "te6cckEBAQEAAgAAAEysuc0="}, TON_SIGNER)),
        ("tron", "Hello"),
    ]
    return messages + [("eip712", text(path)) for path in EIP712_FIXTURES]


def main():
    shutil.rmtree(SEEDS, ignore_errors=True)
    for index, url in enumerate(payment_urls()):
        write("payment_url", f"url_{index:02}", url)
    for index, address in enumerate(addresses()):
        write("address", f"address_{index:02}", address)
    for index, value in enumerate(private_keys()):
        write("private_key", f"key_{index:02}", value)
    for name in KEYSTORE_FIXTURES:
        write("keystore_file", name, fixture(f"crates/gem_keystore/testdata/{name}"))
    write("keystore_file", "v4_header.json", keystore_v4_header())
    for index, (method, chain_id, params) in enumerate(wallet_connect_requests()):
        write("wallet_connect_request", f"{method}_{index:02}", f"{method}\n{chain_id}\napp.example.com\n{params}")
    for index, (name, data) in enumerate(wallet_connect_transactions()):
        write("wallet_connect_transaction", f"{name.replace(':', '_')}_{index:02}", f"{name}\n{data}")
    for index, (name, data) in enumerate(sign_messages()):
        write("sign_message", f"{name}_{index:02}", f"{name}\n{data}")


if __name__ == "__main__":
    main()
