"""Copy a stopped local profile to an independent version; never copy consent."""
import argparse, datetime, json, pathlib, shutil, sqlite3, ctypes, os
from contextlib import closing

def sqlite_database(path):
    if not path.is_file():return False
    with path.open('rb') as file:return file.read(16)==b'SQLite format 3\x00'
def migrate(source,destination,old_version,new_version):
    source=source.resolve();destination=destination.resolve()
    if destination.exists():raise RuntimeError('Destination already exists; preserved')
    lock=source/'.run.lock';handle=None
    if lock.exists() and os.name=='nt':
        kernel=ctypes.WinDLL('kernel32',use_last_error=True)
        kernel.CreateFileW.restype=ctypes.c_void_p
        kernel.CreateFileW.argtypes=[ctypes.c_wchar_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p]
        handle=kernel.CreateFileW(str(lock),0x80000000,0,None,3,0x80,None)
        if handle==ctypes.c_void_p(-1).value:raise RuntimeError('Old profile is running or locked; close it before migration')
    try:
        old=source/'data'/('v'+old_version)
        if not old.is_dir():raise RuntimeError('Source data version missing')
        destination.mkdir(parents=True)
        dest=destination/'data'/('v'+new_version)
        databases=[p for p in old.rglob('*') if sqlite_database(p)]
        database_paths={p.resolve() for p in databases}
        def ignore(directory,names):
            return [name for name in names if name.endswith(('-wal','-shm','.lock')) or name=='rinx-binding-status.json' or (pathlib.Path(directory)/name).resolve() in database_paths]
        shutil.copytree(old,dest,ignore=ignore)
        for db in databases:
            target=dest/db.relative_to(old)
            target.parent.mkdir(parents=True,exist_ok=True)
            with closing(sqlite3.connect('file:'+db.as_posix()+'?mode=ro',uri=True)) as src,closing(sqlite3.connect(target)) as out:
                src.backup(out)
                if out.execute('PRAGMA integrity_check').fetchone()[0]!='ok':raise RuntimeError('Database verification failed')
        if (source/'.secrets').is_dir():shutil.copytree(source/'.secrets',destination/'.secrets')
        if (source/'data/model').is_dir():shutil.copytree(source/'data/model',destination/'data/model')
        record={'source_version':old_version,'destination_version':new_version,'copied_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'original_preserved':True,'effective_consent_inherited':False,'database_integrity':'ok','databases_verified':len(databases)}
        (destination/'migration.local.json').write_text(json.dumps(record,indent=2),'utf8')
        return record
    except Exception:
        if destination.exists():(destination/'migration-failed.local.json').write_text('{"writable":false,"original_preserved":true}','utf8')
        raise
    finally:
        if handle is not None:
            kernel.CloseHandle.argtypes=[ctypes.c_void_p];kernel.CloseHandle(handle)
def main():
    p=argparse.ArgumentParser();p.add_argument('source',type=pathlib.Path);p.add_argument('destination',type=pathlib.Path);p.add_argument('--from-version',required=True);p.add_argument('--to-version',required=True);a=p.parse_args()
    print(json.dumps(migrate(a.source,a.destination,a.from_version,a.to_version)))
if __name__=='__main__':main()
