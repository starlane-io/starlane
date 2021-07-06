/*
Copyright 2021.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package v1alpha1

import (
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

// EDIT THIS FILE!  THIS IS SCAFFOLDING FOR YOU TO OWN!
// NOTE: json tags are required.  Any new fields you add must have json tags for the fields to be serialized.

// StarlaneProvisioningJobSpec defines the desired state of StarlaneProvisioningJob
type StarlaneProvisioningJobSpec struct {
	// INSERT ADDITIONAL SPEC FIELDS - desired state of cluster
	// Important: Run "make" to regenerate code after modifying this file

	ResourceName            string   `json:"resourceName"`
	Provisioner             string   `json:"provisioner"`
	StarlaneResourceAddress string   `json:"starlane-resource-address"`
	InitArgs                []string `json:"init-args,omitempty"`
}

// StarlaneProvisioningJobStatus defines the observed state of StarlaneProvisioningJob
type StarlaneProvisioningJobStatus struct {
	// INSERT ADDITIONAL STATUS FIELD - define observed state of cluster
	// Important: Run "make" to regenerate code after modifying this file
}

//+kubebuilder:object:root=true
//+kubebuilder:subresource:status

// StarlaneProvisioningJob is the Schema for the starlaneprovisioningjobs API
type StarlaneProvisioningJob struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   StarlaneProvisioningJobSpec   `json:"spec,omitempty"`
	Status StarlaneProvisioningJobStatus `json:"status,omitempty"`
}

//+kubebuilder:object:root=true

// StarlaneProvisioningJobList contains a list of StarlaneProvisioningJob
type StarlaneProvisioningJobList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []StarlaneProvisioningJob `json:"items"`
}

func init() {
	SchemeBuilder.Register(&StarlaneProvisioningJob{}, &StarlaneProvisioningJobList{})
}
