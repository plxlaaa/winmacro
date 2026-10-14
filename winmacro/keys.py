from __future__ import annotations

EXTENDED_VKS = frozenset(
    {
        0x21,  # PAGE UP
        0x22,  # PAGE DOWN
        0x23,  # END
        0x24,  # HOME
        0x25,  # LEFT
        0x26,  # UP
        0x27,  # RIGHT
        0x28,  # DOWN
        0x2D,  # INSERT
        0x2E,  # DELETE
        0x5B,  # LWIN
        0x5C,  # RWIN
        0x5D,  # APPS
        0x6F,  # DIVIDE
        0x90,  # NUMLOCK
        0xA3,  # RCONTROL
        0xA5,  # RMENU
    }
)


def is_extended_vk(vk: int | None) -> bool:
    if vk is None:
        return False
    return int(vk) in EXTENDED_VKS
