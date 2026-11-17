#include <mtp/ptp/ObjectFormat.h>
#include <iostream>
#include <cstdlib>
#include <ctime>
int main(int argc,char **argv) { setenv("TZ",argv[1],1);tzset();for(int n=2;n<argc;++n)std::cout<<argv[n]<<" "<<mtp::ConvertDateTime(std::string(argv[n]))<<std::endl; }
