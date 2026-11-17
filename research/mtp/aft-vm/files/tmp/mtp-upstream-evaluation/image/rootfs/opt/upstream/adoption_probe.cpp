// Disposable read-only protocol probe; links the unchanged upstream AFT library.
#include <usb/Context.h>
#include <mtp/ptp/Device.h>
#include <mtp/ptp/Session.h>
#include <mtp/ptp/Messages.h>
#include <iostream>
#include <functional>
#include <sstream>
#include <cstdlib>
void check(const std::string &name, const std::function<void()> &fn) {
 std::cout << "BEGIN " << name << std::endl;
 try { fn(); std::cout << "OK " << name << std::endl; }
 catch(const std::exception &e) { std::cout << "ERROR " << name << " " << e.what() << std::endl; }
}
int main() {
 try {
  mtp::usb::ContextPtr context(new mtp::usb::Context);
  auto device=mtp::Device::FindFirst(context);
  if(!device) throw std::runtime_error("No MTP device");
  auto session=device->OpenSession(1);
  auto storages=session->GetStorageIDs().StorageIDs;
  if(storages.empty()) throw std::runtime_error("No MTP storage");
  auto storage=storages.front();
  auto resolve=[&](const std::string &path) {
   auto parent=mtp::Session::Root; std::istringstream stream(path); std::string part;
   while(std::getline(stream,part,'/')) {
    if(part.empty()) continue;
    bool found=false;
    for(auto id:session->GetObjectHandles(storage,mtp::ObjectFormat::Any,parent).ObjectHandles) {
     if(session->GetObjectStringProperty(id,mtp::ObjectProperty::ObjectFilename)==part) {parent=id;found=true;break;}
    }
    if(!found) throw std::runtime_error("Missing component: "+part);
   }
   return parent;
  };
  auto dir=resolve("Android/data/org.videolan.vlc/files/medialib");
  std::cout << "directory_id " << dir.Id << std::endl;
  check("directory_object_info",[&]{auto x=session->GetObjectInfo(dir);std::cout<<"name "<<x.Filename<<" modified "<<x.ModificationDate<<std::endl;});
  check("children_selected_storage",[&]{auto x=session->GetObjectHandles(storage,mtp::ObjectFormat::Any,dir);std::cout<<"count "<<x.ObjectHandles.size()<<std::endl;});
  check("children_all_storages",[&]{auto x=session->GetObjectHandles(mtp::Session::AllStorages,mtp::ObjectFormat::Any,dir);std::cout<<"count "<<x.ObjectHandles.size()<<std::endl;});
  check("child_filename_property_list",[&]{auto x=session->GetObjectPropertyList(dir,mtp::ObjectFormat::Any,mtp::ObjectProperty::ObjectFilename,0,1);std::cout<<"bytes "<<x.size()<<std::endl;});
  auto file=resolve("ALFS-Finder-Test-20261112-140129/CHEN_5min_MIP04_Master_H264.mp4");
  check("date_modified_capability",[&]{
   auto info=session->GetObjectInfo(file);
   auto desc=session->GetObjectPropertyDesc(mtp::ObjectProperty::DateModified,info.ObjectFormat);
   if(desc.size()<5) throw std::runtime_error("Short property descriptor");
   std::cout<<"property "<<(unsigned(desc[0])|(unsigned(desc[1])<<8))<<" get_set "<<unsigned(desc[4])<<std::endl;
  });
  check("video_raw_date",[&]{
   auto x=session->GetObjectInfo(file);auto p=session->GetObjectStringProperty(file,mtp::ObjectProperty::DateModified);
   std::cout<<"info_modified "<<x.ModificationDate<<" property_modified "<<p<<std::endl;
   for(const char *zone:{"UTC0","CST-8"}) {setenv("TZ",zone,1);tzset();std::cout<<"parsed "<<zone<<" "<<mtp::ConvertDateTime(p)<<std::endl;}
  });
  std::cout<<"closing_session"<<std::endl;session.reset();std::cout<<"closed_session"<<std::endl;
  return 0;
 } catch(const std::exception &e) {std::cerr<<"FATAL "<<e.what()<<std::endl;return 1;}
}
