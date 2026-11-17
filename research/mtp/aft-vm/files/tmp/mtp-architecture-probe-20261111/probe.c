#define FUSE_USE_VERSION 26
#include <fuse.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <sys/time.h>
static double now(void) { struct timeval t; gettimeofday(&t,0); return t.tv_sec+t.tv_usec/1e6; }
static int delayed(const char *op,const char *path) {
 char mode[32]; double until=0; FILE *f=fopen("/tmp/retry-probe/control","r");
 if(!f) return 0;
 int ok=fscanf(f,"%31s %lf",mode,&until); fclose(f);
 int retry=ok==2&&!strcmp(mode,op)&&strstr(path,"slow")&&now()<until;
 if(strstr(path,"slow")) { fprintf(stderr,"%.6f %s %s %s\n",now(),op,path,retry?"EAGAIN":"OK"); fflush(stderr); }
 return retry;
}
static int attrs(const char *p,struct stat *s) {
 if(delayed("getattr",p)) return -EAGAIN;
 memset(s,0,sizeof(*s)); s->st_uid=502; s->st_gid=20; s->st_mtime=1700000000;
 if(!strcmp(p,"/")||!strcmp(p,"/slowdir")) { s->st_mode=S_IFDIR|0555; s->st_nlink=2; return 0; }
 if(!strcmp(p,"/ready.txt")||!strcmp(p,"/slow.txt")||!strcmp(p,"/slowdir/child.txt")) {
  s->st_mode=S_IFREG|0444; s->st_nlink=1; s->st_size=4096; return 0;
 }
 return -ENOENT;
}
static int listing(const char *p,void *b,fuse_fill_dir_t add,off_t o,struct fuse_file_info *fi) {
 (void)o;(void)fi;
 if(delayed("readdir",p)) return -EAGAIN;
 add(b,".",0,0);add(b,"..",0,0);
 if(!strcmp(p,"/")) { add(b,"ready.txt",0,0);add(b,"slow.txt",0,0);add(b,"slowdir",0,0);return 0; }
 if(!strcmp(p,"/slowdir")) { add(b,"child.txt",0,0);return 0; }
 return -ENOENT;
}
static int opening(const char *p,struct fuse_file_info *fi) {
 if(delayed("open",p)) return -EAGAIN;
 fi->direct_io=1; fi->keep_cache=0; return 0;
}
static int reading(const char *p,char *b,size_t n,off_t o,struct fuse_file_info *fi) {
 (void)fi;
 if(delayed("read",p)) return -EAGAIN;
 if(o>=4096) return 0;
 if(n>4096-o) n=4096-o;
 memset(b,'R',n);return n;
}
static int space(const char *p,struct statvfs *s) {
 (void)p; memset(s,0,sizeof(*s));s->f_bsize=s->f_frsize=4096;s->f_blocks=1024;s->f_bfree=s->f_bavail=1024;s->f_namemax=255;return 0;
}
int main(int argc,char **argv) {
 struct fuse_operations ops={.getattr=attrs,.readdir=listing,.open=opening,.read=reading,.statfs=space};
 return fuse_main(argc,argv,&ops,0);
}
