#define _GNU_SOURCE
#include <linux/stat.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <fcntl.h>
#include <stdio.h>
#include <errno.h>
int main(int argc,char **argv){for(int n=1;n<argc;n++){struct statx s={0};errno=0;long r=syscall(SYS_statx,AT_FDCWD,argv[n],0,STATX_BASIC_STATS|STATX_BTIME,&s);printf("%s rc=%ld errno=%d mask=%x birth_supported=%d birth=%lld mtime=%lld ctime=%lld\n",argv[n],r,errno,s.stx_mask,!!(s.stx_mask&STATX_BTIME),(long long)s.stx_btime.tv_sec,(long long)s.stx_mtime.tv_sec,(long long)s.stx_ctime.tv_sec);}return 0;}
