from dataclasses import dataclass, field
from typing import Dict, List

@dataclass
class MutationResult:
    offset: int
    cf_index: int
    cfc_index: int
    vf_index: int
    stable: bool

@dataclass
class Packet:
    length: int
    data: bytes
    mutations: Dict[str, List[MutationResult]] = field(default_factory=dict)

@dataclass
class TestCase:
    sequence_id: int
    packets: List[Packet] = field(default_factory=list)