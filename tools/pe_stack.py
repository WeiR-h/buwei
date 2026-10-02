"""Normalize and verify the unsigned Windows PE main-thread stack after linking.

The reserve is the same 16 MiB requested in .cargo/config.toml. Post-link
normalization avoids target-specific flags being replaced by path-remap flags.
Only the reserve and PE checksum may change; code and resources stay identical.
"""
import argparse,hashlib,json,pathlib,struct

def normalize(file,reserve=16*1024*1024):
    original=file.read_bytes();data=bytearray(original)
    if data[:2]!=b'MZ':raise ValueError('Expected Windows PE')
    header=struct.unpack_from('<I',data,60)[0]
    if data[header:header+4]!=b'PE\0\0' or struct.unpack_from('<H',data,header+4)[0]!=0x8664:raise ValueError('Expected x64 PE')
    optional=header+24
    optional_size=struct.unpack_from('<H',data,header+20)[0]
    if optional_size<152 or optional+optional_size>len(data):raise ValueError('Truncated PE optional header')
    if struct.unpack_from('<H',data,optional)[0]!=0x20b:raise ValueError('Expected PE32+')
    if struct.unpack_from('<I',data,optional+108)[0]<5:raise ValueError('Missing PE security directory')
    if any(struct.unpack_from('<II',data,optional+144)):raise ValueError('Signed binary must not be changed')
    old=struct.unpack_from('<Q',data,optional+72)[0]
    commit=struct.unpack_from('<Q',data,optional+80)[0]
    if reserve<commit or reserve%65536 or reserve<1024*1024:raise ValueError('Invalid stack reserve')
    struct.pack_into('<Q',data,optional+72,reserve)
    checksum_offset=optional+64
    struct.pack_into('<I',data,checksum_offset,0)
    total=0
    for offset in range(0,len(data),2):
        total+=data[offset]+((data[offset+1] if offset+1<len(data) else 0)<<8)
        total=(total&0xffff)+(total>>16)
    checksum=((total&0xffff)+(total>>16))+len(data)
    struct.pack_into('<I',data,checksum_offset,checksum&0xffffffff)
    allowed=set(range(optional+72,optional+80))|set(range(checksum_offset,checksum_offset+4))
    if any(a!=b and i not in allowed for i,(a,b) in enumerate(zip(original,data))):raise AssertionError('Executable code or resources changed')
    if data!=original:file.write_bytes(data)
    return {'stack_reserve_before':old,'stack_reserve_bytes':reserve,'stack_commit_bytes':commit,'code_and_resources_unchanged':True,'unsigned_pe':True,'binary_sha256':hashlib.sha256(data).hexdigest()}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('binary',type=pathlib.Path);a=p.parse_args();print(json.dumps(normalize(a.binary)))
